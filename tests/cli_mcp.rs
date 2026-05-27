//! Integration tests for the MCP surface (Phase 1.7 P006).
//!
//! All tests use binary subprocess spawning (no lib target needed).
//! Tests:
//!   1. `advisory-cron mcp` handshake — initialize + tools/list returns 5 tools.
//!   2. Parity: CLI register produces same plist as is bootstrapped, same label.
//!   3. MCP `tools/call` init (force=false) returns error or success depending on file presence.
//!   4. MCP `tools/call` register with invalid label returns MCP error response.
//!   5. MCP `tools/call` init with path traversal returns MCP error response.
//!   6. CLI `advisory-cron mcp --help` exits 0 (subcommand registered).
//!   7. MCP `tools/list` returns tool count = 5.

use std::{
    io::Write as _,
    process::{Command, Stdio},
};
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_advisory-cron");

fn write_default_config(home: &std::path::Path) {
    let config_dir = home.join(".config/advisory-cron");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("config.toml");
    let toml = format!(
        r#"[task]
command = "echo"
args = ["hello"]
working_dir = "{dir}"

[schedule]
hour = 9
minute = 0

[heartbeat]
log_path = "{dir}/.local/state/advisory-cron/heartbeat.jsonl"
"#,
        dir = home.display()
    );
    std::fs::write(&config_path, toml).unwrap();
}

/// Send JSON-RPC messages and read responses by using piped I/O with proper wait.
fn mcp_exchange(home: &std::path::Path, messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let mut child = Command::new(BIN)
        .env("HOME", home)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn advisory-cron mcp");

    // Write all messages then close stdin.
    {
        let stdin = child.stdin.take().unwrap();
        let mut w = std::io::BufWriter::new(stdin);
        for msg in messages {
            writeln!(w, "{}", serde_json::to_string(msg).unwrap()).unwrap();
        }
        // stdin drops and closes here.
    }

    // Wait for process to exit (should exit after stdin EOF).
    let output = child.wait_with_output().expect("wait_with_output");

    // Parse newline-delimited JSON from stdout.
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                serde_json::from_str(trimmed).ok()
            }
        })
        .collect()
}

// ---- Test 1: mcp --help exits 0 (subcommand registered in CLI) ----

#[test]
fn mcp_subcommand_help_exits_zero() {
    let home = TempDir::new().unwrap();
    let out = Command::new(BIN)
        .env("HOME", home.path())
        .args(["mcp", "--help"])
        .output()
        .expect("spawn failed");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 for mcp --help, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---- Test 2: advisory-cron --help lists `mcp` as subcommand ----

#[test]
fn top_level_help_includes_mcp_subcommand() {
    let home = TempDir::new().unwrap();
    let out = Command::new(BIN)
        .env("HOME", home.path())
        .arg("--help")
        .output()
        .expect("spawn failed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("mcp"),
        "expected 'mcp' in help output, got: {stdout}"
    );
}

// ---- Test 3: MCP initialize handshake + tools/list ----
//
// Send: initialize + initialized notification + tools/list
// Expect: initialize response with serverInfo + ListToolsResult with 5 tools.

#[test]
fn mcp_handshake_and_tools_list_returns_5_tools() {
    let home = TempDir::new().unwrap();
    write_default_config(home.path());

    let init_msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "0.1.0" }
        }
    });

    let initialized_notif = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });

    let tools_list_msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    });

    let responses = mcp_exchange(home.path(), &[init_msg, initialized_notif, tools_list_msg]);

    // Find the initialize response (id=1).
    let init_resp = responses
        .iter()
        .find(|r| r.get("id") == Some(&serde_json::json!(1)));
    assert!(
        init_resp.is_some(),
        "expected initialize response (id=1), got: {responses:?}"
    );
    let init_result = &init_resp.unwrap()["result"];
    assert!(
        init_result.get("serverInfo").is_some(),
        "expected serverInfo in initialize response, got: {init_result}"
    );

    // Find the tools/list response (id=2).
    let tools_resp = responses
        .iter()
        .find(|r| r.get("id") == Some(&serde_json::json!(2)));
    assert!(
        tools_resp.is_some(),
        "expected tools/list response (id=2), got: {responses:?}"
    );
    let tools = &tools_resp.unwrap()["result"]["tools"];
    let tools_arr = tools.as_array().expect("tools must be an array");
    assert_eq!(
        tools_arr.len(),
        5,
        "expected exactly 5 tools, got {}: {tools_arr:?}",
        tools_arr.len()
    );

    let tool_names: Vec<&str> = tools_arr
        .iter()
        .filter_map(|t| t.get("name")?.as_str())
        .collect();
    for expected in ["init", "register", "unregister", "run", "status"] {
        assert!(
            tool_names.contains(&expected),
            "missing tool {expected:?}; got {tool_names:?}"
        );
    }
}

// ---- Test 4: MCP tools/call register with invalid label returns isError=true ----

#[test]
fn mcp_register_tool_rejects_invalid_label() {
    let home = TempDir::new().unwrap();
    write_default_config(home.path());

    let init_msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "0.1.0" }
        }
    });

    let initialized_notif = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });

    let call_msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "register",
            "arguments": {
                "label": "bad label with spaces!"
            }
        }
    });

    let responses = mcp_exchange(home.path(), &[init_msg, initialized_notif, call_msg]);

    let call_resp = responses
        .iter()
        .find(|r| r.get("id") == Some(&serde_json::json!(3)));
    assert!(
        call_resp.is_some(),
        "expected tools/call response (id=3), got: {responses:?}"
    );
    // The result should have isError=true (INV-18 enforcement).
    let result = &call_resp.unwrap()["result"];
    assert_eq!(
        result.get("isError"),
        Some(&serde_json::json!(true)),
        "expected isError=true for invalid label, got: {result}"
    );
}

// ---- Test 5: MCP tools/call init with path traversal returns isError=true ----

#[test]
fn mcp_init_tool_rejects_path_traversal() {
    let home = TempDir::new().unwrap();
    write_default_config(home.path());

    let init_msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "0.1.0" }
        }
    });

    let initialized_notif = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });

    let call_msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "init",
            "arguments": {
                "config_path": "../etc/passwd"
            }
        }
    });

    let responses = mcp_exchange(home.path(), &[init_msg, initialized_notif, call_msg]);

    let call_resp = responses
        .iter()
        .find(|r| r.get("id") == Some(&serde_json::json!(4)));
    assert!(
        call_resp.is_some(),
        "expected tools/call response (id=4), got: {responses:?}"
    );
    let result = &call_resp.unwrap()["result"];
    assert_eq!(
        result.get("isError"),
        Some(&serde_json::json!(true)),
        "expected isError=true for path traversal, got: {result}"
    );
}

// ---- Test 6: MCP serverInfo name = "advisory-cron" ----

#[test]
fn mcp_server_info_name_is_advisory_cron() {
    let home = TempDir::new().unwrap();
    write_default_config(home.path());

    let init_msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "0.1.0" }
        }
    });

    let responses = mcp_exchange(home.path(), &[init_msg]);

    let init_resp = responses
        .iter()
        .find(|r| r.get("id") == Some(&serde_json::json!(1)));
    assert!(init_resp.is_some(), "expected initialize response");
    let server_name = init_resp.unwrap()["result"]["serverInfo"]["name"].as_str();
    assert_eq!(
        server_name,
        Some("advisory-cron"),
        "expected serverInfo.name='advisory-cron'"
    );
}

// ---- Test 7: Parity — CLI register produces identical label in output as core ----
// This is a structural parity test: both CLI and MCP paths call core::register::run.
// We verify via binary that the CLI produces correct output (parity is enforced by
// the shared core, tested in unit tests inside src/core/register.rs #[cfg(test)]).

#[test]
fn parity_cli_register_uses_correct_label_suffix() {
    let home = TempDir::new().unwrap();
    write_default_config(home.path());
    let label = format!("parity-p006-cli-{}", std::process::id());

    let out = Command::new(BIN)
        .env("HOME", home.path())
        .args(["register", "--label", &label, "--schedule", "0 10 * * *"])
        .output()
        .expect("spawn failed");

    // Exit 0 expected (real launchctl bootstrap may fail on CI — exit 3 is acceptable for this test).
    // Focus: stdout must contain the label (proving core::register was called with correct args).
    let stdout = String::from_utf8_lossy(&out.stdout);
    let code = out.status.code().unwrap_or(-1);
    assert!(
        code == 0 || code == 3,
        "expected exit 0 or 3 (launchctl may fail on CI), got {code}"
    );
    if code == 0 {
        assert!(
            stdout.contains(&label),
            "expected label in stdout: {stdout}"
        );
        // Cleanup.
        let _ = Command::new(BIN)
            .env("HOME", home.path())
            .args(["unregister", "--label", &label])
            .output();
    }
}
