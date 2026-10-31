use crate::{auth::Auth, error::Result, state::AppState};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

#[derive(Deserialize, Serialize, Clone)]
pub struct Preset {
    pub id: Uuid,
    pub name: String,
    pub category: String,
    pub language: String,
    pub subject: String,
    pub html: String,
    pub text: String,
    pub variables: Map<String, Value>,
}

pub fn presets() -> Vec<Preset> {
    let mut presets: Vec<Preset> = serde_json::from_str(include_str!("../emails/presets.json"))
        .expect("bundled email presets must be valid");
    for preset in &mut presets {
        preset.html = include_str!("../emails/layout.html")
            .replace("<!--LANG-->", &preset.language)
            .replace("<!--CONTENT-->", &preset.html)
            .replace(
                "<!--FOOTER-->",
                if preset.language == "zh-CN" {
                    "此邮件由 {{ brand }} 发送。"
                } else {
                    "Sent by {{ brand }}."
                },
            );
    }
    presets
}

pub async fn insert_missing(
    connection: &mut PgConnection,
) -> std::result::Result<u64, sqlx::Error> {
    let mut count = 0;
    for preset in presets() {
        count += sqlx::query("INSERT INTO templates (id,name,subject,html,text) VALUES ($1,$2,$3,$4,$5) ON CONFLICT (id) DO NOTHING")
            .bind(preset.id).bind(preset.name).bind(preset.subject).bind(preset.html).bind(preset.text)
            .execute(&mut *connection).await?.rows_affected();
    }
    Ok(count)
}

pub async fn initialize(auth: Auth, State(state): State<AppState>) -> Result<Json<Value>> {
    auth.require_admin()?;
    let mut tx = state.db.begin().await?;
    let inserted = insert_missing(&mut tx).await?;
    tx.commit().await?;
    Ok(Json(json!({"inserted": inserted})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{SendRequest, Template},
        send::render_template,
    };

    #[test]
    fn presets_render_with_sample_values_in_both_languages() {
        let presets = presets();
        let unique = presets
            .iter()
            .map(|p| p.id)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), 8);
        for preset in presets {
            let template = Template {
                id: preset.id,
                name: preset.name,
                subject: preset.subject,
                html: preset.html,
                text: preset.text,
                updated_at: chrono::Utc::now(),
            };
            let mut input = SendRequest {
                variables: preset.variables,
                ..Default::default()
            };
            render_template(&template, &mut input).unwrap();
            assert!(!input.subject.is_empty());
            assert!(!input.html.contains("{{"));
            assert!(!input.text.contains("{{"));
            assert!(
                input
                    .html
                    .contains(&format!("lang=\"{}\"", preset.language))
            );
        }
    }
}
