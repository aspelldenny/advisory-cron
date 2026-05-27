//! MCP tool definitions and handlers.
//!
//! `AdvisoryCronHandler` implements rmcp's `ServerHandler` trait.
//! Each tool handler validates inputs (INV-18), then delegates to `core::*::run`.
//! Hand-written JSON schemas per Decision 3 (no `#[derive(JsonSchema)]` on our types needed).

use std::sync::Arc;

use rmcp::{
    ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
        ServerInfo, Tool,
    },
    service::RequestContext,
};

use crate::core;
use crate::launchd::RealLaunchctl;

/// MCP server handler — stateless, `Default`-derived.
#[derive(Default)]
pub struct AdvisoryCronHandler;

/// Build `serde_json::Map` from a `serde_json::Value::Object`.
/// Panics if the value is not an object — only called with literal JSON objects.
fn json_schema(v: serde_json::Value) -> Arc<serde_json::Map<String, serde_json::Value>> {
    Arc::new(v.as_object().cloned().expect("schema must be JSON object"))
}

/// Validate label allowlist (INV-18 MCP boundary enforcement).
fn validate_label(label: &str) -> Result<(), String> {
    if label.is_empty()
        || !label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Err(format!(
            "invalid label {:?} — must be ASCII alphanumeric + '-' + '_'",
            label
        ))
    } else {
        Ok(())
    }
}

/// Validate config_path field (INV-18 path traversal check).
fn validate_config_path(s: &str) -> Result<std::path::PathBuf, String> {
    let p = std::path::PathBuf::from(s);
    if p.components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        Err(format!(
            "config_path must not contain '..' traversal: {:?}",
            s
        ))
    } else {
        Ok(p)
    }
}

/// Build a tool error CallToolResult (is_error=true, message as text content).
fn tool_error(msg: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![Content::text(msg.into())])
}

/// Build a tool success CallToolResult (serialized JSON as text content).
fn tool_ok<T: serde::Serialize>(v: &T) -> CallToolResult {
    match serde_json::to_string_pretty(v) {
        Ok(json) => CallToolResult::success(vec![Content::text(json)]),
        Err(e) => tool_error(format!("failed to serialize output: {e}")),
    }
}

fn make_tools() -> Vec<Tool> {
    vec![
        Tool::new(
            "init",
            "Write a default advisory-cron config to ~/.config/advisory-cron/config.toml \
             (or path specified). Refuses overwrite unless force=true.",
            json_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "force": {
                        "type": "boolean",
                        "description": "Overwrite existing config",
                        "default": false
                    },
                    "config_path": {
                        "type": "string",
                        "description": "Optional override path"
                    }
                }
            })),
        ),
        Tool::new(
            "register",
            "Generate a launchd plist and bootstrap it for the given label. \
             Schedule in M H * * * form (daily) overrides config schedule if provided.",
            json_schema(serde_json::json!({
                "type": "object",
                "required": ["label"],
                "properties": {
                    "label": {
                        "type": "string",
                        "description": "Label (ASCII alphanumeric + -_)"
                    },
                    "schedule": {
                        "type": "string",
                        "description": "Cron M H * * * form (daily only)"
                    },
                    "config_path": {
                        "type": "string"
                    }
                }
            })),
        ),
        Tool::new(
            "unregister",
            "Boot out the launchd job for the given label and remove the plist file. \
             Idempotent.",
            json_schema(serde_json::json!({
                "type": "object",
                "required": ["label"],
                "properties": {
                    "label": {
                        "type": "string"
                    },
                    "config_path": {
                        "type": "string"
                    }
                }
            })),
        ),
        Tool::new(
            "run",
            "Fire the configured task once; capture stdout/stderr; append heartbeat. \
             Returns exit_code, duration_ms, tails.",
            json_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "config_path": {
                        "type": "string"
                    }
                }
            })),
        ),
        Tool::new(
            "status",
            "Read launchd state + last N heartbeats. Returns plist_loaded, next_fire \
             (configured recurrence), heartbeat_log_path, last_runs.",
            json_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "label": {
                        "type": "string"
                    },
                    "config_path": {
                        "type": "string"
                    },
                    "last": {
                        "type": "integer",
                        "minimum": 0,
                        "default": 5
                    }
                }
            })),
        ),
    ]
}

impl ServerHandler for AdvisoryCronHandler {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.server_info = Implementation::new("advisory-cron", env!("CARGO_PKG_VERSION"));
        info
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<rmcp::service::RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        Ok(ListToolsResult {
            tools: make_tools(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<rmcp::service::RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(dispatch_tool(request).await)
    }
}

/// Dispatch a tool call by name. Returns `CallToolResult` (never Err — errors
/// are represented as is_error=true in the result per MCP protocol).
async fn dispatch_tool(request: CallToolRequestParams) -> CallToolResult {
    let args = request.arguments.unwrap_or_default();

    match request.name.as_ref() {
        "init" => handle_init(args),
        "register" => handle_register(args),
        "unregister" => handle_unregister(args),
        "run" => handle_run(args).await,
        "status" => handle_status(args),
        other => tool_error(format!("unknown tool: {other:?}")),
    }
}

fn handle_init(args: serde_json::Map<String, serde_json::Value>) -> CallToolResult {
    let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

    let config_path = match args.get("config_path").and_then(|v| v.as_str()) {
        Some(s) => match validate_config_path(s) {
            Ok(p) => Some(p),
            Err(e) => return tool_error(e),
        },
        None => None,
    };

    match core::init::run(core::init::InitArgs { force, config_path }) {
        Ok(output) => tool_ok(&output),
        Err(e) => tool_error(format!("{e:#}")),
    }
}

fn handle_register(args: serde_json::Map<String, serde_json::Value>) -> CallToolResult {
    let label = match args.get("label").and_then(|v| v.as_str()) {
        Some(l) => l.to_string(),
        None => return tool_error("missing required field: label"),
    };
    // INV-18 MCP boundary label validation.
    if let Err(e) = validate_label(&label) {
        return tool_error(e);
    }

    let schedule = args
        .get("schedule")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let config_path = match args.get("config_path").and_then(|v| v.as_str()) {
        Some(s) => match validate_config_path(s) {
            Ok(p) => Some(p),
            Err(e) => return tool_error(e),
        },
        None => None,
    };

    let client = RealLaunchctl;
    match core::register::run(
        core::register::RegisterArgs {
            label,
            schedule,
            config_path,
        },
        &client,
    ) {
        Ok(output) => tool_ok(&output),
        Err(e) => tool_error(format!("{e:#}")),
    }
}

fn handle_unregister(args: serde_json::Map<String, serde_json::Value>) -> CallToolResult {
    let label = match args.get("label").and_then(|v| v.as_str()) {
        Some(l) => l.to_string(),
        None => return tool_error("missing required field: label"),
    };
    // INV-18 MCP boundary label validation.
    if let Err(e) = validate_label(&label) {
        return tool_error(e);
    }

    let config_path = match args.get("config_path").and_then(|v| v.as_str()) {
        Some(s) => match validate_config_path(s) {
            Ok(p) => Some(p),
            Err(e) => return tool_error(e),
        },
        None => None,
    };

    let client = RealLaunchctl;
    match core::unregister::run(
        core::unregister::UnregisterArgs { label, config_path },
        &client,
    ) {
        Ok(output) => tool_ok(&output),
        Err(e) => tool_error(format!("{e:#}")),
    }
}

async fn handle_run(args: serde_json::Map<String, serde_json::Value>) -> CallToolResult {
    let config_path = match args.get("config_path").and_then(|v| v.as_str()) {
        Some(s) => match validate_config_path(s) {
            Ok(p) => Some(p),
            Err(e) => return tool_error(e),
        },
        None => None,
    };

    match core::run::run(core::run::RunArgs { config_path }).await {
        Ok(output) => tool_ok(&output),
        Err(e) => tool_error(format!("{e:#}")),
    }
}

fn handle_status(args: serde_json::Map<String, serde_json::Value>) -> CallToolResult {
    let label = args
        .get("label")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // INV-18: validate label if provided (collapsible_if: merged with None guard below).
    if matches!(&label, Some(l) if validate_label(l).is_err()) {
        let l = label.as_deref().unwrap();
        return tool_error(validate_label(l).unwrap_err());
    }

    let config_path = match args.get("config_path").and_then(|v| v.as_str()) {
        Some(s) => match validate_config_path(s) {
            Ok(p) => Some(p),
            Err(e) => return tool_error(e),
        },
        None => None,
    };

    let last = args
        .get("last")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(5);

    let client = RealLaunchctl;
    match core::status::run(
        core::status::StatusArgs {
            label,
            config_path,
            last,
        },
        &client,
    ) {
        Ok(report) => tool_ok(&report),
        Err(e) => tool_error(format!("{e:#}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_label_accepts_valid() {
        assert!(validate_label("advisory-scan").is_ok());
        assert!(validate_label("test_label_1").is_ok());
    }

    #[test]
    fn validate_label_rejects_invalid() {
        assert!(validate_label("").is_err());
        assert!(validate_label("bad label").is_err());
        assert!(validate_label("../etc/passwd").is_err());
    }

    #[test]
    fn validate_config_path_rejects_traversal() {
        assert!(validate_config_path("../etc/passwd").is_err());
        assert!(validate_config_path("/some/path/../etc").is_err());
    }

    #[test]
    fn validate_config_path_accepts_normal() {
        assert!(validate_config_path("/home/user/.config/advisory-cron/config.toml").is_ok());
    }

    #[test]
    fn make_tools_returns_5_tools() {
        let tools = make_tools();
        assert_eq!(tools.len(), 5);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"init"));
        assert!(names.contains(&"register"));
        assert!(names.contains(&"unregister"));
        assert!(names.contains(&"run"));
        assert!(names.contains(&"status"));
    }

    #[test]
    fn handle_init_missing_home_returns_error_result() {
        let saved = std::env::var("HOME").ok();
        unsafe {
            std::env::remove_var("HOME");
        }
        let result = handle_init(Default::default());
        if let Some(h) = saved {
            unsafe {
                std::env::set_var("HOME", h);
            }
        }
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn dispatch_unknown_tool_returns_error() {
        let req = CallToolRequestParams::new("nonexistent-tool");
        let result = tokio_test::block_on(dispatch_tool(req));
        assert_eq!(result.is_error, Some(true));
    }
}
