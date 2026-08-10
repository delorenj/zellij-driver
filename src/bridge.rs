//! Authenticated HTTP-to-Zellij navigation bridge for Agentboard.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use axum::{
    extract::State,
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc, time::Duration};
use subtle::ConstantTimeEq;
use tokio::{process::Command, time::timeout};

const ZELLIJ_ACTION_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NavigationAction {
    TabPrevious,
    TabNext,
    PaneUp,
    PaneDown,
    PaneLeft,
    PaneRight,
}

impl NavigationAction {
    fn cli_args(self) -> &'static [&'static str] {
        match self {
            Self::TabPrevious => &["go-to-previous-tab"],
            Self::TabNext => &["go-to-next-tab"],
            Self::PaneUp => &["move-focus", "up"],
            Self::PaneDown => &["move-focus", "down"],
            Self::PaneLeft => &["move-focus-or-tab", "left"],
            Self::PaneRight => &["move-focus-or-tab", "right"],
        }
    }
}

#[async_trait]
pub trait ActionExecutor: Send + Sync {
    async fn execute(&self, action: NavigationAction) -> Result<()>;
}

#[derive(Clone)]
pub struct ZellijCliExecutor {
    binary: PathBuf,
    session: Arc<str>,
    action_timeout: Duration,
}

impl ZellijCliExecutor {
    pub fn new(binary: impl Into<PathBuf>, session: impl Into<Arc<str>>) -> Self {
        Self {
            binary: binary.into(),
            session: session.into(),
            action_timeout: ZELLIJ_ACTION_TIMEOUT,
        }
    }
}

#[async_trait]
impl ActionExecutor for ZellijCliExecutor {
    async fn execute(&self, action: NavigationAction) -> Result<()> {
        let mut command = Command::new(&self.binary);
        command
            .env_remove("ZELLIJ")
            .env_remove("ZELLIJ_SESSION_NAME")
            .arg("--session")
            .arg(self.session.as_ref())
            .arg("action")
            .args(action.cli_args())
            .kill_on_drop(true);

        let output = timeout(self.action_timeout, command.output())
            .await
            .map_err(|_| anyhow!("Zellij action timed out after five seconds"))?
            .context("failed to execute Zellij")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Zellij rejected the action: {}", stderr.trim()));
        }

        Ok(())
    }
}

#[derive(Clone)]
struct BridgeState {
    executor: Arc<dyn ActionExecutor>,
    token: Arc<[u8]>,
}

#[derive(Debug, Deserialize)]
struct ActionRequest {
    action: NavigationAction,
}

#[derive(Debug, Serialize)]
struct ActionResponse {
    ok: bool,
    action: NavigationAction,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: &'static str,
}

pub fn router(executor: Arc<dyn ActionExecutor>, token: impl AsRef<[u8]>) -> Router {
    let state = BridgeState {
        executor,
        token: Arc::from(token.as_ref()),
    };

    Router::new()
        .route("/healthz", get(health))
        .route("/v1/action", post(run_action))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

async fn run_action(
    State(state): State<BridgeState>,
    headers: HeaderMap,
    Json(request): Json<ActionRequest>,
) -> Response {
    if !has_valid_token(&headers, &state.token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "unauthorized",
            }),
        )
            .into_response();
    }

    match state.executor.execute(request.action).await {
        Ok(()) => (
            StatusCode::OK,
            Json(ActionResponse {
                ok: true,
                action: request.action,
            }),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(action = ?request.action, error = %error, "Zellij bridge action failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: "zellij_action_failed",
                }),
            )
                .into_response()
        }
    }
}

fn has_valid_token(headers: &HeaderMap, expected: &[u8]) -> bool {
    let Some(value) = headers.get(AUTHORIZATION) else {
        return false;
    };
    let Ok(value) = value.to_str() else {
        return false;
    };
    let Some(provided) = value.strip_prefix("Bearer ") else {
        return false;
    };

    provided.as_bytes().ct_eq(expected).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use std::sync::Mutex;
    use tower::ServiceExt;

    const TOKEN: &str = "test-token-with-at-least-thirty-two-bytes";

    #[derive(Default)]
    struct RecordingExecutor {
        actions: Mutex<Vec<NavigationAction>>,
    }

    #[async_trait]
    impl ActionExecutor for RecordingExecutor {
        async fn execute(&self, action: NavigationAction) -> Result<()> {
            self.actions
                .lock()
                .expect("action mutex poisoned")
                .push(action);
            Ok(())
        }
    }

    #[test]
    fn maps_only_the_supported_zellij_actions() {
        assert_eq!(
            NavigationAction::TabPrevious.cli_args(),
            &["go-to-previous-tab"]
        );
        assert_eq!(NavigationAction::TabNext.cli_args(), &["go-to-next-tab"]);
        assert_eq!(NavigationAction::PaneUp.cli_args(), &["move-focus", "up"]);
        assert_eq!(
            NavigationAction::PaneDown.cli_args(),
            &["move-focus", "down"]
        );
        assert_eq!(
            NavigationAction::PaneLeft.cli_args(),
            &["move-focus-or-tab", "left"]
        );
        assert_eq!(
            NavigationAction::PaneRight.cli_args(),
            &["move-focus-or-tab", "right"]
        );
    }

    #[tokio::test]
    async fn rejects_requests_without_the_bearer_token() {
        let executor = Arc::new(RecordingExecutor::default());
        let response = router(executor.clone(), TOKEN)
            .oneshot(action_request("tab-next", None))
            .await
            .expect("router failed");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(executor
            .actions
            .lock()
            .expect("action mutex poisoned")
            .is_empty());
    }

    #[tokio::test]
    async fn authenticated_request_executes_exactly_one_action() {
        let executor = Arc::new(RecordingExecutor::default());
        let response = router(executor.clone(), TOKEN)
            .oneshot(action_request("pane-left", Some(TOKEN)))
            .await
            .expect("router failed");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            *executor.actions.lock().expect("action mutex poisoned"),
            vec![NavigationAction::PaneLeft],
        );
    }

    #[tokio::test]
    async fn unknown_actions_never_reach_the_executor() {
        let executor = Arc::new(RecordingExecutor::default());
        let response = router(executor.clone(), TOKEN)
            .oneshot(action_request("run-arbitrary-command", Some(TOKEN)))
            .await
            .expect("router failed");

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(executor
            .actions
            .lock()
            .expect("action mutex poisoned")
            .is_empty());
    }

    fn action_request(action: &str, token: Option<&str>) -> Request<Body> {
        let mut request = Request::builder()
            .method("POST")
            .uri("/v1/action")
            .header("content-type", "application/json");
        if let Some(token) = token {
            request = request.header(AUTHORIZATION, format!("Bearer {token}"));
        }
        request
            .body(Body::from(format!(r#"{{"action":"{action}"}}"#)))
            .expect("request should build")
    }
}
