use anyhow::Result;
use bunmail_rs::{auth, config::Config, crypto, model::Domain, queue, smtp, state::AppState, web};
use hickory_resolver::TokioAsyncResolver;
use sqlx::postgres::PgPoolOptions;
use std::{
    collections::HashMap,
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::Duration,
};

fn healthcheck() -> Result<()> {
    let mut stream =
        std::net::TcpStream::connect_timeout(&"127.0.0.1:3000".parse()?, Duration::from_secs(3))?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
    let mut buf = String::new();
    stream.read_to_string(&mut buf)?;
    anyhow::ensure!(buf.starts_with("HTTP/1.1 200"), "Unhealthy");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args().any(|a| a == "--healthcheck") {
        return healthcheck();
    }
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "bunmail_rs=info".into()),
        )
        .init();
    let mut config = Config::from_env()?;
    bunmail_rs::send::validate_address(&config.owner_email)
        .map_err(|_| anyhow::anyhow!("Invalid OWNER_EMAIL"))?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&config.database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    let mut state = AppState {
        cloudflare_lock: Default::default(),
        updates: Default::default(),
        db: pool,
        config: Arc::new(config.clone()),
        dns: TokioAsyncResolver::tokio_from_system_conf()?,
        limits: Arc::new(Mutex::new(HashMap::new())),
    };
    auth::bootstrap(&state).await?;
    if std::env::args().any(|a| a == "--reset-password") {
        let mut password = String::new();
        std::io::stdin().take(258).read_to_string(&mut password)?;
        auth::reset_password(&state, password.trim_end_matches(['\r', '\n']).to_owned()).await?;
        println!("Password reset; all browser sessions revoked");
        state.db.close().await;
        return Ok(());
    }
    config.bootstrap_password = None;
    state.config = Arc::new(config);
    for domain in sqlx::query_as::<_, Domain>("SELECT * FROM domains")
        .fetch_all(&state.db)
        .await?
    {
        let _private = zeroize::Zeroizing::new(crypto::decrypt(
            &state.config.encryption_key,
            &domain.name,
            &domain.private_key,
        )?);
    }
    let listener = tokio::net::TcpListener::bind(state.config.bind).await?;
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let smtp = if state.config.smtp_bind.is_some() {
        Some(smtp::start(state.clone(), stop_rx.clone()).await?)
    } else {
        None
    };
    let worker = tokio::spawn(queue::run(state.clone(), stop_rx));
    tracing::info!(address=%state.config.bind,version=env!("CARGO_PKG_VERSION"),delivery=state.delivery_enabled().await?,"BunMail RS started");
    let signal_tx = stop_tx.clone();
    axum::serve(
        listener,
        web::router(state.clone()).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        #[cfg(unix)]
        {
            let mut terminate =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("signal handler");
            tokio::select! {_=tokio::signal::ctrl_c()=>{},_=terminate.recv()=>{}}
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        let _ = signal_tx.send(true);
    })
    .await?;
    let _ = stop_tx.send(true);
    let _ = tokio::time::timeout(Duration::from_secs(130), worker).await;
    if let Some(smtp) = smtp {
        smtp.abort();
    }
    state.db.close().await;
    Ok(())
}
