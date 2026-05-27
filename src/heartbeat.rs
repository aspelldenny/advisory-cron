//! Phase 1.4 — heartbeat JSONL log writer + reader.
//!
//! Schema is DURABLE — Phase 2 alert + P005 status both consume. Adding fields requires
//! `schema_version` bump per ARCHITECTURE.md §Heartbeat schema. Field order in this struct
//! definition matches doc spec line-by-line.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// One heartbeat = one task fire result, serialized as 1 JSON line.
///
/// Schema spec: `docs/ARCHITECTURE.md` §Heartbeat schema. Field order MUST match.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct HeartbeatRecord {
    pub ts: DateTime<Utc>,
    pub label: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub stdout_tail: String,
    pub stderr_tail: String,
}

/// Append one heartbeat record as a single JSON line to `log_path`.
///
/// Creates parent directory if missing (per Heads-up #4 — `~/.local/state/advisory-cron/`
/// may not exist on fresh install). Creates the file if missing. Append-only — never
/// truncates or rotates (PROJECT.md hard line #4 "Heartbeat log is append-only").
pub fn append(log_path: &Path, record: &HeartbeatRecord) -> Result<()> {
    if let Some(parent) = log_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create heartbeat dir {parent:?}"))?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .with_context(|| format!("failed to open heartbeat file {log_path:?} for append"))?;

    let line = serde_json::to_string(record).context("failed to serialize HeartbeatRecord")?;
    file.write_all(line.as_bytes())
        .with_context(|| format!("failed to write heartbeat line to {log_path:?}"))?;
    file.write_all(b"\n")
        .with_context(|| format!("failed to write newline to {log_path:?}"))?;

    Ok(())
}

/// Read the last `n` heartbeat records (oldest-first within the returned Vec).
///
/// Returns `Ok(vec![])` if the file does not exist (no fires yet — distinguish from read error).
/// Malformed lines are skipped with a stderr warning; continuing parsing — defensive against
/// partial-write corruption (P004 does NOT use crash-safe write+rename; Phase 2.3 will).
///
/// Called by Phase 1.5 `status` subcommand — forward-declared here.
#[allow(dead_code)]
pub fn read_last_n(log_path: &Path, n: usize) -> Result<Vec<HeartbeatRecord>> {
    if !log_path.exists() {
        return Ok(vec![]);
    }

    let file = fs::File::open(log_path)
        .with_context(|| format!("failed to open heartbeat file {log_path:?} for read"))?;
    let reader = BufReader::new(file);

    let mut records: Vec<HeartbeatRecord> = Vec::new();
    for (i, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("failed to read line {i} of {log_path:?}"))?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<HeartbeatRecord>(&line) {
            Ok(rec) => records.push(rec),
            Err(err) => {
                eprintln!("warning: skipping malformed heartbeat line {i}: {err}");
            }
        }
    }

    let start = records.len().saturating_sub(n);
    Ok(records.into_iter().skip(start).collect())
}

/// Truncate `s` to the last `max_bytes` bytes, snapping to a UTF-8 character boundary
/// (NOT grapheme cluster — that would need `unicode-segmentation` dep).
///
/// Returns owned String. If `s.len() <= max_bytes`, returns full copy.
///
/// Note: snaps to char boundary only, not grapheme cluster boundary. A multi-codepoint
/// grapheme (e.g. emoji + skin-tone modifier) may be split at the codepoint level.
/// Acceptable for diagnostic readability — advisory-cron is not a display engine.
pub(crate) fn tail_utf8(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    // Start from the byte index (len - max_bytes), walk forward to next char boundary.
    let mut start = s.len() - max_bytes;
    while start < s.len() && !s.is_char_boundary(start) {
        start += 1;
    }
    s[start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn sample_record() -> HeartbeatRecord {
        HeartbeatRecord {
            ts: Utc.with_ymd_and_hms(2026, 5, 27, 2, 0, 0).unwrap(),
            label: "advisory-scan-daily".to_string(),
            exit_code: 0,
            duration_ms: 45230,
            stdout_tail: "last 1KB of stdout".to_string(),
            stderr_tail: "".to_string(),
        }
    }

    #[test]
    fn heartbeat_record_serde_roundtrip() {
        let rec = sample_record();
        let json = serde_json::to_string(&rec).unwrap();
        let parsed: HeartbeatRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(rec, parsed);
        // Confirm schema field names appear verbatim
        assert!(json.contains("\"ts\":"));
        assert!(json.contains("\"label\":"));
        assert!(json.contains("\"exit_code\":"));
        assert!(json.contains("\"duration_ms\":"));
        assert!(json.contains("\"stdout_tail\":"));
        assert!(json.contains("\"stderr_tail\":"));
    }

    #[test]
    fn append_creates_parent_dir_and_file() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("a/b/c/heartbeat.jsonl");
        let rec = sample_record();
        append(&nested, &rec).expect("append should create parents + file");
        assert!(nested.exists());
        let contents = fs::read_to_string(&nested).unwrap();
        assert_eq!(contents.lines().count(), 1);
        assert!(
            contents.ends_with('\n'),
            "trailing newline required for JSONL"
        );
    }

    #[test]
    fn append_then_read_last_n_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("heartbeat.jsonl");
        for i in 0..5 {
            let mut rec = sample_record();
            rec.exit_code = i;
            append(&path, &rec).unwrap();
        }
        let last3 = read_last_n(&path, 3).unwrap();
        assert_eq!(last3.len(), 3);
        assert_eq!(last3[0].exit_code, 2);
        assert_eq!(last3[2].exit_code, 4);
    }

    #[test]
    fn read_last_n_missing_file_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("does-not-exist.jsonl");
        let result = read_last_n(&path, 10).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn read_last_n_skips_malformed_line() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("heartbeat.jsonl");
        let rec = sample_record();
        append(&path, &rec).unwrap();
        // Inject a bad line
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"this-is-not-json\n").unwrap();
        append(&path, &rec).unwrap();
        let recs = read_last_n(&path, 10).unwrap();
        assert_eq!(recs.len(), 2, "should skip malformed line, keep 2 valid");
    }

    #[test]
    fn tail_utf8_under_limit_returns_full() {
        let s = "hello";
        assert_eq!(tail_utf8(s, 1024), "hello");
    }

    #[test]
    fn tail_utf8_over_limit_truncates_to_char_boundary() {
        // "héllo" — é is 2 bytes (0xC3 0xA9). Build a string where a naive byte-cut would split it.
        let s = "aaaaaaaaaaé"; // 10 ASCII + 2 bytes for é = 12 bytes total
        // Request last 3 bytes — naive cut at index 9 lands inside é. Must snap forward.
        let tail = tail_utf8(s, 3);
        assert!(
            std::str::from_utf8(tail.as_bytes()).is_ok(),
            "must be valid UTF-8"
        );
        // Should NOT contain a half-é. Either contains é fully (snap-forward landed at é boundary)
        // or skips é (snap-forward to byte 12 = empty).
        assert!(
            tail.is_empty() || tail == "é" || tail == "a" || tail == "aé",
            "unexpected tail: {tail:?}"
        );
    }

    #[test]
    fn tail_utf8_pure_ascii_exact_cut() {
        let s = "0123456789";
        assert_eq!(tail_utf8(s, 4), "6789");
    }
}
