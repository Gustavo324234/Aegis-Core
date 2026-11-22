pub mod admin;
pub mod agents;
pub mod antigravity_bridge;
pub mod auth;
pub mod chat_history;
pub mod engine;
pub mod fs;
pub mod mcp_api;
pub mod memory_api;
pub mod music_api;
pub mod oauth_api;
pub mod openapi;
pub mod persona_api;
pub mod personal;
pub mod providers;
pub mod prs;
pub mod router_api;
pub mod siren_api;
pub mod status;
pub mod stt_download;
pub mod system_config_api;
pub mod train;
pub mod video_api;
pub mod whatsapp_api;
pub mod workspace;

use crate::static_files;
use crate::ws;
use crate::AppState;
use axum::{
    http::{
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
        HeaderName, HeaderValue, Method, Uri,
    },
    routing::get,
    Router,
};
use tower_http::cors::{AllowOrigin, CorsLayer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

fn parse_origin_only(raw: &str) -> Option<HeaderValue> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() || trimmed == "*" {
        return None;
    }
    let uri: Uri = trimmed.parse().ok()?;
    let scheme = uri.scheme_str()?;
    if !matches!(scheme, "http" | "https") {
        return None;
    }
    let authority = uri.authority()?.as_str();
    let normalized = format!("{}://{}", scheme, authority);
    HeaderValue::from_str(&normalized).ok()
}

fn push_unique_origin(list: &mut Vec<HeaderValue>, candidate: Option<HeaderValue>) {
    if let Some(origin) = candidate {
        if !list.contains(&origin) {
            list.push(origin);
        }
    }
}

pub(crate) fn build_cors_layer_with(
    port: u16,
    extra_origins_csv: Option<&str>,
    base_url: Option<&str>,
    domain: Option<&str>,
    tls_enabled: bool,
) -> CorsLayer {
    let mut allowed_origins: Vec<HeaderValue> = Vec::new();

    for raw in [
        format!("http://localhost:{port}"),
        format!("http://127.0.0.1:{port}"),
        format!("http://[::1]:{port}"),
    ] {
        push_unique_origin(&mut allowed_origins, parse_origin_only(&raw));
    }

    if tls_enabled {
        for raw in [
            format!("https://localhost:{port}"),
            format!("https://127.0.0.1:{port}"),
            format!("https://[::1]:{port}"),
        ] {
            push_unique_origin(&mut allowed_origins, parse_origin_only(&raw));
        }
    }

    if cfg!(debug_assertions) {
        for raw in ["http://localhost:5173", "http://127.0.0.1:5173"] {
            push_unique_origin(&mut allowed_origins, parse_origin_only(raw));
        }
    }

    if let Some(url) = base_url {
        push_unique_origin(&mut allowed_origins, parse_origin_only(url));
    }

    if let Some(dom) = domain {
        let trimmed = dom.trim();
        if !trimmed.is_empty() {
            let candidate = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                trimmed.to_string()
            } else {
                format!("https://{trimmed}")
            };
            push_unique_origin(&mut allowed_origins, parse_origin_only(&candidate));
        }
    }

    if let Some(csv) = extra_origins_csv {
        for item in csv.split(',') {
            push_unique_origin(&mut allowed_origins, parse_origin_only(item));
        }
    }

    CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            AUTHORIZATION,
            CONTENT_TYPE,
            ACCEPT,
            HeaderName::from_static("x-citadel-tenant"),
            HeaderName::from_static("x-citadel-key"),
            HeaderName::from_static("x-citadel-token"),
            HeaderName::from_static("x-aegis-hook-token"),
        ])
}

fn build_cors_layer(port: u16) -> CorsLayer {
    let extra = std::env::var("AEGIS_CORS_ORIGINS").ok();
    let base_url = std::env::var("AEGIS_BASE_URL").ok();
    let domain = std::env::var("AEGIS_DOMAIN").ok();
    let tls_enabled = std::env::var("AEGIS_TLS_CERT").is_ok();
    build_cors_layer_with(
        port,
        extra.as_deref(),
        base_url.as_deref(),
        domain.as_deref(),
        tls_enabled,
    )
}

pub fn build_router(state: AppState) -> Router {
    let swagger_ui =
        SwaggerUi::new("/api/docs/:_").url("/api-docs/openapi.json", openapi::ApiDoc::openapi());

    let cors = build_cors_layer(state.config.port);

    Router::new()
        .route("/health", get(status::health_check))
        .merge(swagger_ui)
        // API Routes
        .nest("/api/auth", auth::router())
        .nest("/api/admin", admin::router())
        .nest("/api/admin/system-config", system_config_api::router())
        .nest("/api/engine", engine::router())
        .nest("/api/oauth", oauth_api::router())
        .nest("/api/whatsapp", whatsapp_api::router())
        .nest("/api/router", router_api::router())
        .nest("/api/chat", chat_history::router())
        .nest("/api/workspace", workspace::router())
        .nest("/api/fs", fs::router())
        .nest("/api/prs", prs::router())
        .nest("/api/git", prs::git_router())
        .nest("/api/providers", providers::router())
        .nest("/api/status", status::router())
        .nest("/api/system", status::system_router())
        .nest("/api/siren", siren_api::router())
        .nest("/api/siren/stt", stt_download::router())
        .nest("/api/persona", persona_api::router())
        .nest("/api/agents", agents::router())
        .nest("/api/antigravity", antigravity_bridge::router())
        .nest("/api/music", music_api::router())
        .nest("/api/train", train::router())
        .nest("/api/video", video_api::router())
        .nest("/api/mcp", mcp_api::router())
        .nest("/api/memory", memory_api::router())
        .nest("/api/personal", personal::router())
        .route("/api/chronos/events", get(personal::get_chronos_events))
        // WebSocket Routes
        .nest("/ws/chat", ws::chat::router())
        .nest("/ws/siren", ws::siren::router())
        .nest("/ws/agents", ws::agents::router())
        // Static Files (Catch-all)
        .fallback(static_files::spa_handler)
        .layer(cors)
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{header, Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn cors_blocks_arbitrary_origin_and_allows_explicit_list() -> anyhow::Result<()> {
        let app = Router::new()
            .route("/health", get(|| async { StatusCode::OK }))
            .layer(build_cors_layer_with(
                8000,
                Some("https://panel.aegis.local, *"),
                None,
                None,
                false,
            ));

        let evil_get = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/health")
                    .header(header::ORIGIN, "https://evil.example")
                    .body(Body::empty())?,
            )
            .await?;
        assert!(evil_get
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none());

        let evil_preflight = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/health")
                    .header(header::ORIGIN, "https://attacker.example")
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                    .body(Body::empty())?,
            )
            .await?;
        assert!(evil_preflight
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none());

        let local_ok = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/health")
                    .header(header::ORIGIN, "http://localhost:8000")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(
            local_ok
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .and_then(|v| v.to_str().ok()),
            Some("http://localhost:8000")
        );

        let extra_ok = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/health")
                    .header(header::ORIGIN, "https://panel.aegis.local")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(
            extra_ok
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .and_then(|v| v.to_str().ok()),
            Some("https://panel.aegis.local")
        );

        Ok(())
    }
}
