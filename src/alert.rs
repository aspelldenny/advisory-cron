//! Telegram alert sender (Phase 2.1).
//!
//! Best-effort outbound POST to Telegram Bot API. Per PROJECT.md hard line #5
//! ("Failure mode = noisy"), advisory-cron surfaces task failures to Sếp's
//! phone via Telegram. Alert failure does NOT fail the task (logged via
//! `tracing::warn!`, swallowed at the caller).
//!
//! INV-19 governs this module: explicit timeout + error handling.
//!
//! **Env-free contract (V2 — per Worker Turn 1 recommendation):**
//! This module MUST NOT call `std::env::var` for any reason except `HOME`
//! inside `expand_home` (a unix filesystem primitive). The API base test seam
//! env var (see INV-19) is read at the call site in `src/core/run.rs` and
//! passed in via `send_with_base(api_base, msg)`. This keeps `alert.rs` a
//! pure function of its inputs and unit-testable without env setup.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use crate::config::TelegramConfig;

// Production base URL — used by `send()` convenience wrapper.
// `send()` and `new()` are public API kept for call sites that don't need the
// test-seam override. Currently only exercised in unit tests; suppress the
// dead_code lint until a future Phase 2.x call site uses them directly.
#[allow(dead_code)]
const TELEGRAM_API_BASE: &str = "https://api.telegram.org";
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct TelegramAlert {
    bot_token: String,
    chat_id: String,
}

impl TelegramAlert {
    /// Build from config. Returns `Ok(None)` if `[alert.telegram]` block absent.
    /// Returns `Err` if config malformed (e.g. `bot_token_file` set but file
    /// unreadable, or neither `bot_token` nor `bot_token_file` provided).
    pub fn from_config(cfg: Option<&TelegramConfig>) -> Result<Option<Self>> {
        let Some(tg) = cfg else {
            return Ok(None);
        };
        let token = resolve_token(tg)?;
        Ok(Some(Self {
            bot_token: token,
            chat_id: tg.chat_id.clone(),
        }))
    }

    /// Construct from raw token + chat_id (test helper / explicit override).
    // Used in unit tests; allow dead_code in binary crate context.
    #[allow(dead_code)]
    pub fn new(bot_token: impl Into<String>, chat_id: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            chat_id: chat_id.into(),
        }
    }

    /// Convenience wrapper — forwards to `send_with_base` with the production
    /// Telegram API URL. Production call sites in `core::run::run` MUST use
    /// `send_with_base(api_base, msg)` directly (with `api_base` resolved from
    /// the API base override env var at the call site — see INV-19).
    /// This `send` shim exists for any future non-test call site that does
    /// not need the env override.
    // Allow dead_code: currently all call sites go through send_with_base.
    // send() is kept as a stable public API for future use.
    #[allow(dead_code)]
    pub async fn send(&self, message: &str) -> Result<()> {
        self.send_with_base(TELEGRAM_API_BASE, message).await
    }

    /// Send a message. `api_base` is the URL scheme+host (no trailing slash),
    /// e.g. `"https://api.telegram.org"` (prod) or the wiremock URL (tests).
    /// No env reads inside this function — `api_base` is the explicit seam.
    pub async fn send_with_base(&self, api_base: &str, message: &str) -> Result<()> {
        let url = format!("{api_base}/bot{}/sendMessage", self.bot_token);
        let client = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .context("build reqwest client")?;
        let resp = tokio::time::timeout(
            HTTP_TIMEOUT,
            client
                .post(&url)
                .form(&[("chat_id", self.chat_id.as_str()), ("text", message)])
                .send(),
        )
        .await
        .context("telegram POST timed out")?
        .context("telegram POST transport error")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("telegram API error: status={status} body={body}");
        }
        Ok(())
    }
}

fn resolve_token(tg: &TelegramConfig) -> Result<String> {
    match (&tg.bot_token, &tg.bot_token_file) {
        (Some(t), None) => Ok(t.clone()),
        (None, Some(p)) => read_token_from_file(p),
        (Some(_), Some(_)) => {
            anyhow::bail!(
                "[alert.telegram]: provide either `bot_token` or `bot_token_file`, not both"
            )
        }
        (None, None) => {
            anyhow::bail!("[alert.telegram]: missing both `bot_token` and `bot_token_file`")
        }
    }
}

/// Read `KEY=VAL` lines from `path`. Extract `TG_BOT_TOKEN=...` value.
/// Expands leading `~/` to `$HOME` (the only env read allowed in alert.rs —
/// HOME path expansion is a unix filesystem primitive, not a test seam).
fn read_token_from_file(path: &Path) -> Result<String> {
    let expanded = expand_home(path)?;
    let content = std::fs::read_to_string(&expanded)
        .with_context(|| format!("read bot_token_file {}", expanded.display()))?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("TG_BOT_TOKEN=") {
            // Strip optional surrounding quotes.
            let val = rest.trim_matches(|c| c == '"' || c == '\'');
            if val.is_empty() {
                anyhow::bail!("TG_BOT_TOKEN is empty in {}", expanded.display());
            }
            return Ok(val.to_string());
        }
    }
    anyhow::bail!("TG_BOT_TOKEN not found in {}", expanded.display())
}

fn expand_home(path: &Path) -> Result<PathBuf> {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/") {
        let home = std::env::var("HOME")
            .context("HOME env var unset — required to expand `~/` in bot_token_file")?;
        if home.is_empty() {
            anyhow::bail!("HOME is empty");
        }
        return Ok(PathBuf::from(home).join(rest));
    }
    Ok(path.to_path_buf())
}

/// Format the alert message body sent to Telegram on task failure.
/// Caller pre-truncates stderr_tail to ~500 bytes to keep total message
/// under Telegram's 4096-char limit.
pub fn format_failure_message(
    label: &str,
    exit_code: i32,
    duration_ms: u64,
    stderr_tail: &str,
) -> String {
    let tail = if stderr_tail.is_empty() {
        "<no stderr>".to_string()
    } else {
        // Truncate to ~500 bytes at UTF-8 char boundary.
        truncate_bytes(stderr_tail, 500)
    };
    format!(
        "advisory-cron failed\nlabel={label}\nexit_code={exit_code}\nduration_ms={duration_ms}\n\nstderr_tail:\n{tail}"
    )
}

fn truncate_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = s[..end].to_string();
    out.push('\u{2026}'); // ellipsis
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AlertConfig, TelegramConfig};
    use std::io::Write;
    use tempfile::TempDir;

    // ---------------------------------------------------------------------------
    // from_config tests
    // ---------------------------------------------------------------------------

    #[test]
    fn from_config_none_returns_ok_none() {
        let result = TelegramAlert::from_config(None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn from_config_inline_token_returns_ok_some() {
        let tg = TelegramConfig {
            chat_id: "123".to_string(),
            bot_token: Some("mytoken".to_string()),
            bot_token_file: None,
        };
        let alert = TelegramAlert::from_config(Some(&tg)).unwrap().unwrap();
        assert_eq!(alert.chat_id, "123");
        assert_eq!(alert.bot_token, "mytoken");
    }

    #[test]
    fn from_config_file_token_reads_file() {
        let dir = TempDir::new().unwrap();
        let secrets_path = dir.path().join("secrets.env");
        let mut f = std::fs::File::create(&secrets_path).unwrap();
        writeln!(f, "# comment").unwrap();
        writeln!(f, "TG_BOT_TOKEN=filetoken123").unwrap();
        writeln!(f, "TG_CHAT_ID=999").unwrap();

        let tg = TelegramConfig {
            chat_id: "456".to_string(),
            bot_token: None,
            bot_token_file: Some(secrets_path),
        };
        let alert = TelegramAlert::from_config(Some(&tg)).unwrap().unwrap();
        assert_eq!(alert.bot_token, "filetoken123");
        assert_eq!(alert.chat_id, "456");
    }

    #[test]
    fn from_config_both_set_returns_err() {
        let tg = TelegramConfig {
            chat_id: "123".to_string(),
            bot_token: Some("tok".to_string()),
            bot_token_file: Some("/tmp/file".into()),
        };
        let err = TelegramAlert::from_config(Some(&tg)).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("not both"), "got: {msg}");
    }

    #[test]
    fn from_config_neither_set_returns_err() {
        let tg = TelegramConfig {
            chat_id: "123".to_string(),
            bot_token: None,
            bot_token_file: None,
        };
        let err = TelegramAlert::from_config(Some(&tg)).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("missing both"), "got: {msg}");
    }

    // ---------------------------------------------------------------------------
    // alert_config helper — build an AlertConfig with inline token
    // ---------------------------------------------------------------------------

    fn make_alert_cfg_inline(token: &str, chat_id: &str) -> AlertConfig {
        AlertConfig {
            telegram: Some(TelegramConfig {
                chat_id: chat_id.to_string(),
                bot_token: Some(token.to_string()),
                bot_token_file: None,
            }),
        }
    }

    // ---------------------------------------------------------------------------
    // send_with_base tests (wiremock)
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn send_with_base_happy_200() {
        use wiremock::matchers::{method, path_regex};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path_regex(r"/bot.*/sendMessage"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"ok":true,"result":{"message_id":1}}"#),
            )
            .mount(&server)
            .await;

        let alert = TelegramAlert::new("testtoken", "123");
        alert
            .send_with_base(&server.uri(), "hello")
            .await
            .expect("200 should succeed");
    }

    #[tokio::test]
    async fn send_with_base_500_returns_err() {
        use wiremock::matchers::{method, path_regex};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path_regex(r"/bot.*/sendMessage"))
            .respond_with(
                ResponseTemplate::new(500).set_body_string(r#"{"ok":false,"description":"err"}"#),
            )
            .mount(&server)
            .await;

        let alert = TelegramAlert::new("testtoken", "123");
        let err = alert
            .send_with_base(&server.uri(), "hello")
            .await
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("status=500") || msg.contains("500"),
            "got: {msg}"
        );
    }

    #[tokio::test]
    async fn send_with_base_401_returns_err_with_body() {
        use wiremock::matchers::{method, path_regex};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path_regex(r"/bot.*/sendMessage"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_string(r#"{"ok":false,"description":"Unauthorized"}"#),
            )
            .mount(&server)
            .await;

        let alert = TelegramAlert::new("badtoken", "123");
        let err = alert
            .send_with_base(&server.uri(), "fail")
            .await
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("401") || msg.contains("status"), "got: {msg}");
    }

    // ---------------------------------------------------------------------------
    // format_failure_message tests
    // ---------------------------------------------------------------------------

    #[test]
    fn format_failure_message_contains_fields() {
        let msg = format_failure_message("my-task", 1, 500, "stderr content");
        assert!(msg.contains("my-task"), "label missing: {msg}");
        assert!(msg.contains("exit_code=1"), "exit_code missing: {msg}");
        assert!(
            msg.contains("duration_ms=500"),
            "duration_ms missing: {msg}"
        );
        assert!(msg.contains("stderr content"), "stderr missing: {msg}");
    }

    #[test]
    fn format_failure_message_empty_stderr() {
        let msg = format_failure_message("lbl", 2, 100, "");
        assert!(
            msg.contains("<no stderr>"),
            "empty-stderr sentinel missing: {msg}"
        );
    }

    // ---------------------------------------------------------------------------
    // truncate_bytes UTF-8 boundary test
    // ---------------------------------------------------------------------------

    #[test]
    fn truncate_bytes_respects_utf8_boundary() {
        // "안" = 3 bytes in UTF-8. Build a string slightly over 500 bytes.
        let korean = "안".repeat(170); // 510 bytes
        let truncated = truncate_bytes(&korean, 500);
        // Must be valid UTF-8 (no panic) and end with ellipsis.
        assert!(truncated.ends_with('\u{2026}'));
        // Must be <= 500 bytes in the non-ellipsis part.
        let without_ellipsis = &truncated[..truncated.len() - '\u{2026}'.len_utf8()];
        assert!(without_ellipsis.len() <= 500);
    }

    // ---------------------------------------------------------------------------
    // AlertConfig used in config module — ensure it round-trips via config tests
    // (main config tests cover load_without_alert; here we use it for from_config)
    // ---------------------------------------------------------------------------

    #[test]
    fn alert_config_telegram_none_means_no_alert() {
        let cfg = AlertConfig { telegram: None };
        let result = TelegramAlert::from_config(cfg.telegram.as_ref()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn alert_config_inline_token_resolves() {
        let cfg = make_alert_cfg_inline("tok123", "chatid456");
        let tg = cfg.telegram.as_ref().unwrap();
        let alert = TelegramAlert::from_config(Some(tg)).unwrap().unwrap();
        assert_eq!(alert.bot_token, "tok123");
        assert_eq!(alert.chat_id, "chatid456");
    }
}
