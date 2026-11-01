use crate::{
    auth::Auth,
    error::{AppError, Result},
    model::{SendRequest, Template},
    send,
    state::AppState,
};
use axum::{
    Form,
    extract::State,
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct PreviewInput {
    template_id: Option<Uuid>,
    #[serde(default)]
    subject: String,
    #[serde(default)]
    html: String,
    #[serde(default)]
    text: String,
    #[serde(default = "empty_variables")]
    variables: String,
}
fn empty_variables() -> String {
    "{}".into()
}

async fn render(state: &AppState, input: PreviewInput) -> Result<String> {
    if input.html.len() + input.text.len() > 512 * 1024 || input.variables.len() > 64 * 1024 {
        return Err(AppError::invalid("Preview exceeds size limits"));
    }
    let template = if let Some(id) = input.template_id {
        sqlx::query_as::<_, Template>("SELECT * FROM templates WHERE id=$1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(AppError::not_found)?
    } else {
        Template {
            id: Uuid::nil(),
            name: String::new(),
            subject: input.subject,
            html: input.html,
            text: input.text,
            updated_at: chrono::Utc::now(),
        }
    };
    let mut message = SendRequest {
        variables: serde_json::from_str(&input.variables)
            .map_err(|_| AppError::invalid("Use valid JSON object for variables"))?,
        ..Default::default()
    };
    send::render_template(&template, &mut message)?;
    if message.html.is_empty() {
        Ok(format!(
            "<!doctype html><meta charset=utf-8><pre style=\"white-space:pre-wrap;font:14px/1.8 sans-serif;padding:24px;overflow-wrap:anywhere\">{}</pre>",
            html_escape::encode_text(&message.text)
        ))
    } else {
        Ok(message.html)
    }
}

pub async fn preview(
    auth: Auth,
    State(state): State<AppState>,
    Form(input): Form<PreviewInput>,
) -> Result<Response> {
    auth.require_admin()?;
    let (status, html) = match render(&state, input).await {
        Ok(html) => (StatusCode::OK, html),
        Err(error) => (
            error.0,
            format!(
                "<!doctype html><meta charset=utf-8><p style=\"padding:24px;font:14px/1.8 sans-serif;color:#a03540\">{}</p>",
                html_escape::encode_text(&error.2)
            ),
        ),
    };
    // Email HTML is untrusted. The preview has no scripts, network resources, forms or same-origin access.
    Ok((status, [
        (header::CONTENT_SECURITY_POLICY,"default-src 'none'; style-src 'unsafe-inline'; sandbox; base-uri 'none'; form-action 'none'; frame-ancestors 'self'"),
        (header::X_FRAME_OPTIONS,"SAMEORIGIN"),
    ],Html(html)).into_response())
}
