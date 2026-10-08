//! Lectura de credenciales de Claude Code y consulta del endpoint de uso.

use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const OAUTH_BETA: &str = "oauth-2025-04-20";

#[derive(Debug, Clone)]
pub struct Limit {
    pub label: String,
    pub percent: f64,
    pub resets_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct Usage {
    pub session: Option<Limit>,
    pub weekly: Option<Limit>,
    /// Límites semanales por modelo u otros que devuelva la API.
    pub others: Vec<Limit>,
    pub plan: Option<String>,
}

#[derive(Deserialize)]
struct Credentials {
    #[serde(rename = "claudeAiOauth")]
    oauth: OAuth,
}

#[derive(Deserialize)]
struct OAuth {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
}

#[derive(Deserialize)]
struct UsageResponse {
    five_hour: Option<Window>,
    seven_day: Option<Window>,
    #[serde(default)]
    limits: Vec<ApiLimit>,
}

#[derive(Deserialize)]
struct Window {
    utilization: Option<f64>,
    resets_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
struct ApiLimit {
    kind: String,
    percent: Option<f64>,
    resets_at: Option<DateTime<Utc>>,
    scope: Option<Scope>,
}

#[derive(Deserialize)]
struct Scope {
    model: Option<Named>,
    surface: Option<Named>,
}

#[derive(Deserialize)]
struct Named {
    display_name: Option<String>,
}

fn credentials_path() -> PathBuf {
    let dir = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".claude")
        });
    dir.join(".credentials.json")
}

/// Se relee en cada consulta porque Claude Code renueva el token periódicamente.
fn read_credentials() -> Result<OAuth, String> {
    let path = credentials_path();
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("No se pudo leer {}: {e}", path.display()))?;
    let creds: Credentials =
        serde_json::from_str(&raw).map_err(|e| format!("Credenciales no válidas: {e}"))?;
    Ok(creds.oauth)
}

pub fn fetch() -> Result<Usage, String> {
    let oauth = read_credentials()?;

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .user_agent(concat!(
            "claude-usage-indicator/",
            env!("CARGO_PKG_VERSION")
        ))
        .build();

    let resp: UsageResponse = match agent
        .get(USAGE_URL)
        .set("Authorization", &format!("Bearer {}", oauth.access_token))
        .set("anthropic-beta", OAUTH_BETA)
        .call()
    {
        Ok(r) => r
            .into_json()
            .map_err(|e| format!("Respuesta no válida: {e}"))?,
        Err(ureq::Error::Status(401, _)) => {
            return Err("Token caducado: abre Claude Code para renovarlo".into());
        }
        Err(ureq::Error::Status(429, _)) => {
            return Err("Demasiadas consultas (429), se reintentará".into());
        }
        Err(ureq::Error::Status(code, _)) => return Err(format!("Error HTTP {code}")),
        Err(e) => return Err(format!("Error de red: {e}")),
    };

    Ok(to_usage(resp, oauth.subscription_type))
}

fn to_usage(resp: UsageResponse, plan: Option<String>) -> Usage {
    let mut usage = Usage {
        session: None,
        weekly: None,
        others: Vec::new(),
        plan,
    };

    for l in resp.limits {
        let Some(percent) = l.percent else { continue };
        let limit = |label: String| Limit {
            label,
            percent,
            resets_at: l.resets_at,
        };
        match l.kind.as_str() {
            "session" => usage.session = Some(limit("Sesión (5 h)".into())),
            "weekly_all" => usage.weekly = Some(limit("Semanal".into())),
            _ => {
                let name = l
                    .scope
                    .and_then(|s| s.model.or(s.surface))
                    .and_then(|n| n.display_name)
                    .unwrap_or(l.kind);
                usage.others.push(limit(format!("Semanal · {name}")));
            }
        }
    }

    // Compatibilidad con respuestas sin el campo `limits`.
    let from_window = |w: Window, label: &str| {
        w.utilization.map(|percent| Limit {
            label: label.into(),
            percent,
            resets_at: w.resets_at,
        })
    };
    if usage.session.is_none() {
        usage.session = resp.five_hour.and_then(|w| from_window(w, "Sesión (5 h)"));
    }
    if usage.weekly.is_none() {
        usage.weekly = resp.seven_day.and_then(|w| from_window(w, "Semanal"));
    }

    usage
}
