//! Phase 1.4 — heartbeat JSONL log writer + reader.
//!
//! Schema is DURABLE — Phase 2 alert + P005 status both consume. Adding fields requires
//! `schema_version` bump per ARCHITECTURE.md §Heartbeat schema. Field order in this struct
//! definition matches doc spec line-by-line.
//!
//! Phase 2.3 (P010): `append` refactored to atomic temp+fsync+rename protocol (INV-21).
//! `read_last_n` tightened: last-line corrupt → warn+skip; mid-file corrupt → fail loud.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tracing::warn;

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
/// Phase 2.3 — Atomic temp+fsync+rename protocol (INV-21). Each call either fully
/// appends a well-formed line OR leaves the file unchanged. No observable partial state.
///
/// Creates parent directory if missing (INV-15). Append-only semantics preserved
/// (PROJECT.md hard line #4) — never compacts, rotates, or reorders.
pub fn append(log_path: &Path, record: &HeartbeatRecord) -> Result<()> {
    // Phase 2.3 — Crash-safe atomic-rename write protocol (P010, INV-21).
    //
    // Replaces the Phase 1.4 direct-append (`OpenOptions::append(true)` + write)
    // implementation. The previous impl was not crash-safe: a kill mid-write
    // could leave the JSONL file with a truncated final line. Under P009 retry
    // policy, a single `advisory-cron run` invocation may call this fn 3+ times
    // (once per retry attempt), tripling the crash-surface area. This impl
    // guarantees that each call either fully appends a well-formed line OR
    // leaves the file unchanged — there is no observable partial state.
    //
    // Protocol:
    //   1. Ensure parent dir exists (carry-over from Phase 1.4; INV-15).
    //   2. Read existing file contents into memory (empty if file absent).
    //   3. Serialize the new record to a JSONL line (record + `\n`).
    //   4. Append the new line to the in-memory buffer.
    //   5. Create a NamedTempFile in the SAME directory as the target file
    //      (atomic rename requires same filesystem).
    //   6. Write the full buffer to the temp file.
    //   7. fsync the temp file (sync_all — data + metadata, so file size is
    //      durable across power loss).
    //   8. Atomically persist (rename) the temp file over the target file.
    //
    // If any step fails before the persist call, the temp file is auto-cleaned
    // on Drop and the target file is untouched. Caller (`core::run::run`)
    // already log-warn-continues on `Err` per P004 contract — task is NOT
    // failed on heartbeat write failure (heartbeat is observability, not the
    // task outcome itself).

    // Step 1 — ensure parent dir (Phase 1.4 carry-over, INV-15).
    if let Some(parent) = log_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating parent dir for {}", log_path.display()))?;
    }

    // Step 2 — read existing contents (empty if file absent).
    let mut buffer = match fs::read(log_path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", log_path.display())),
    };

    // Step 3 — serialize new record.
    let line = serde_json::to_string(record).context("serializing HeartbeatRecord to JSON")?;

    // Step 4 — append to in-memory buffer.
    buffer.extend_from_slice(line.as_bytes());
    buffer.push(b'\n');

    // Step 5 — create temp file in same directory (required for atomic rename).
    let parent_dir = log_path.parent().unwrap_or_else(|| Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent_dir)
        .with_context(|| format!("creating temp file in {}", parent_dir.display()))?;

    // Step 6 — write full buffer.
    use std::io::Write;
    temp.write_all(&buffer)
        .with_context(|| format!("writing to temp file {}", temp.path().display()))?;

    // Step 7 — fsync (sync_all = data + metadata so file size is durable).
    temp.as_file()
        .sync_all()
        .with_context(|| format!("fsyncing temp file {}", temp.path().display()))?;

    // Step 8 — atomic rename. `persist` calls `std::fs::rename` under the
    // hood, which is atomic on POSIX for same-filesystem renames.
    temp.persist(log_path)
        .map_err(|e| anyhow::anyhow!("atomic rename to {} failed: {}", log_path.display(), e))?;

    Ok(())
}

/// Read the last `n` heartbeat records (oldest-first within the returned Vec).
///
/// Phase 2.3 — Partial-last-line tolerance (INV-21 read-path):
/// - Returns `Ok(vec![])` if the file does not exist.
/// - Last line parse failure → `tracing::warn!` + skip + return prior records.
/// - Non-last line parse failure → propagate as `Err` (mid-file corruption is
///   unexpected under the atomic-write protocol — must surface loud per
///   PROJECT.md hard line #5).
/// - Blank lines are tolerated silently (skipped).
pub fn read_last_n(log_path: &Path, n: usize) -> Result<Vec<HeartbeatRecord>> {
    // Phase 2.3 — Partial-last-line tolerance (P010, INV-21 read-path).
    //
    // Pre-P010 heartbeat files may contain a truncated last line from a
    // historical interrupted write (the Phase 1.4 direct-append impl was not
    // crash-safe — see `append` for the new atomic-write protocol). The read
    // path must tolerate ONE such partial line at the END of the file.
    //
    // Tolerance policy:
    //   - Last line parse failure → `tracing::warn!` + skip + continue. This
    //     is the recovery path for legacy partial writes.
    //   - Non-last line parse failure → `Err`. Mid-file corruption is
    //     unexpected (atomic-write prevents it going forward, and a partial
    //     line can only be the LAST line by construction of how truncation
    //     works). If we see mid-file corruption, something else is wrong
    //     (external tampering, disk failure) and we MUST surface it loud.
    //
    // V2 NOTE: This REPLACES the prior P004 behavior of silently skipping ALL
    // malformed lines (via stderr print + continue). The prior behavior masked
    // mid-file corruption — violation of PROJECT.md hard line #5. Existing test
    // `read_last_n_skips_malformed_line` is updated to assert the new
    // last-vs-mid distinction.

    use std::io::{BufRead, BufReader};

    let file = match fs::File::open(log_path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("opening {}", log_path.display())),
    };

    let reader = BufReader::new(file);
    let raw_lines: Vec<String> = reader
        .lines()
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("reading lines from {}", log_path.display()))?;

    let mut records: Vec<HeartbeatRecord> = Vec::with_capacity(raw_lines.len());
    let last_idx = raw_lines.len().saturating_sub(1);

    for (idx, line) in raw_lines.iter().enumerate() {
        if line.trim().is_empty() {
            // Tolerate blank lines silently (could appear if a previous write
            // wrote ONLY the trailing newline and was interrupted before the
            // record bytes — extremely rare with the new atomic protocol but
            // defensive). Blank lines are never records.
            continue;
        }
        match serde_json::from_str::<HeartbeatRecord>(line) {
            Ok(rec) => records.push(rec),
            Err(parse_err) if idx == last_idx => {
                warn!(
                    log_path = %log_path.display(),
                    error = %parse_err,
                    "partial or corrupt last heartbeat line detected (likely pre-P010 interrupted write); skipping"
                );
                // Skip this line, do not propagate error.
            }
            Err(parse_err) => {
                // Mid-file corruption — propagate. Should not happen with the
                // atomic-write protocol; indicates external tampering or disk
                // damage.
                return Err(parse_err).with_context(|| {
                    format!(
                        "parsing heartbeat line {} of {} (mid-file corruption — non-last-line parse failure)",
                        idx + 1,
                        log_path.display()
                    )
                });
            }
        }
    }

    // Take last `n` records (preserve chronological order — oldest of the
    // returned slice first, newest last; matches the existing P004 contract
    // per P005 status reporter expectation).
    if records.len() > n {
        let skip = records.len() - n;
        Ok(records.into_iter().skip(skip).collect())
    } else {
        Ok(records)
    }
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

    /// Two-param helper for tests that need varied records (P010 new tests).
    fn make_record(i: u32, label: &str) -> HeartbeatRecord {
        HeartbeatRecord {
            ts: chrono::Utc::now(),
            label: label.to_string(),
            exit_code: i as i32,
            duration_ms: 100 + i as u64,
            stdout_tail: format!("stdout for {i}"),
            stderr_tail: String::new(),
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

    /// V2 semantic flip: mid-file corrupt line MUST fail loud (INV-21 sub-rule 2).
    /// Previously this test asserted skip-all; now it asserts is_err() for mid-file.
    #[test]
    fn read_last_n_skips_malformed_line() {
        use std::io::Write;
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("heartbeat.jsonl");
        let rec = sample_record();

        // Write a good line, then a bad mid-file line, then another good line.
        // Under V2 protocol, the bad line is NOT the last → must fail loud.
        let good_json = serde_json::to_string(&rec).unwrap();
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "{good_json}").unwrap();
        writeln!(f, "this-is-not-json").unwrap();
        writeln!(f, "{good_json}").unwrap();

        let result = read_last_n(&path, 10);
        assert!(
            result.is_err(),
            "mid-file corrupt line MUST propagate as Err per INV-21 sub-rule 2"
        );
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

    // --- P010 new tests (Task 5) ---

    #[test]
    fn append_creates_file_when_missing() {
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("heartbeat.jsonl");
        assert!(!log_path.exists());

        let rec = make_record(0, "test-create");
        append(&log_path, &rec).expect("append should create the file");
        assert!(
            log_path.exists(),
            "heartbeat file must exist after first append"
        );
        let contents = std::fs::read_to_string(&log_path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 1, "exactly 1 line after 1 append");
        let parsed: HeartbeatRecord = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(parsed.label, "test-create");
    }

    #[test]
    fn append_preserves_existing_content() {
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("heartbeat.jsonl");

        let rec1 = make_record(0, "test-first");
        let rec2 = make_record(1, "test-second");
        append(&log_path, &rec1).unwrap();
        append(&log_path, &rec2).unwrap();

        let contents = std::fs::read_to_string(&log_path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2, "atomic append must preserve prior line");
        let p1: HeartbeatRecord = serde_json::from_str(lines[0]).unwrap();
        let p2: HeartbeatRecord = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(p1.label, "test-first");
        assert_eq!(p2.label, "test-second");
    }

    #[test]
    fn append_multiple_times_grows_file_monotonically() {
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("heartbeat.jsonl");

        let mut last_size: u64 = 0;
        for i in 0..5 {
            let rec = make_record(i, &format!("test-{i}"));
            append(&log_path, &rec).unwrap();
            let size = std::fs::metadata(&log_path).unwrap().len();
            assert!(
                size > last_size,
                "file size must grow monotonically (iter {i}: {last_size} → {size})"
            );
            last_size = size;
        }
        let lines: Vec<String> = std::fs::read_to_string(&log_path)
            .unwrap()
            .lines()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(lines.len(), 5, "5 appends = 5 lines");
    }

    #[test]
    fn append_leaves_no_temp_file_in_parent_dir() {
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("heartbeat.jsonl");

        append(&log_path, &make_record(0, "noleak-1")).unwrap();
        append(&log_path, &make_record(1, "noleak-2")).unwrap();
        append(&log_path, &make_record(2, "noleak-3")).unwrap();

        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        // Only the heartbeat.jsonl itself should remain — no `.tmp*` leftovers.
        assert_eq!(
            entries.len(),
            1,
            "only heartbeat.jsonl should remain, found: {entries:?}"
        );
        assert_eq!(entries[0], "heartbeat.jsonl");
    }

    #[test]
    fn read_last_n_with_corrupt_last_line_skips_it_and_returns_prior() {
        use std::io::Write;
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("heartbeat.jsonl");

        // Append 2 good records via the atomic protocol.
        append(&log_path, &make_record(0, "good-1")).unwrap();
        append(&log_path, &make_record(1, "good-2")).unwrap();

        // Manually append a corrupt trailing line (simulating pre-P010 partial write).
        {
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&log_path)
                .unwrap();
            // Truncated JSON, no trailing newline — simulates crash mid-write.
            f.write_all(b"{\"ts\":\"2026-05-27T00:00:00").unwrap();
        }

        let records = read_last_n(&log_path, 10)
            .expect("corrupt LAST line must be tolerated, not propagated");
        assert_eq!(
            records.len(),
            2,
            "2 prior good records returned, corrupt last line skipped"
        );
        assert_eq!(records[0].label, "good-1");
        assert_eq!(records[1].label, "good-2");
    }

    #[test]
    fn read_last_n_with_corrupt_mid_line_fails_loud() {
        use std::io::Write;
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("heartbeat.jsonl");

        // Manually craft a file with: good-line, corrupt-line, good-line.
        {
            let good1 = serde_json::to_string(&make_record(0, "g1")).unwrap();
            let good2 = serde_json::to_string(&make_record(1, "g2")).unwrap();
            let mut f = std::fs::File::create(&log_path).unwrap();
            writeln!(f, "{good1}").unwrap();
            writeln!(f, "{{this is not json}}").unwrap();
            writeln!(f, "{good2}").unwrap();
        }

        let result = read_last_n(&log_path, 10);
        assert!(
            result.is_err(),
            "mid-file corruption MUST fail loud per INV-21 sub-rule 2"
        );
    }

    #[test]
    fn read_last_n_returns_empty_on_missing_file() {
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("does-not-exist.jsonl");
        let records = read_last_n(&log_path, 5).expect("missing file should return Ok(empty)");
        assert!(records.is_empty());
    }

    #[test]
    fn read_last_n_skips_blank_lines_silently() {
        use std::io::Write;
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("heartbeat.jsonl");
        {
            let good = serde_json::to_string(&make_record(0, "g-blank")).unwrap();
            let mut f = std::fs::File::create(&log_path).unwrap();
            writeln!(f).unwrap(); // blank line
            writeln!(f, "{good}").unwrap();
            writeln!(f).unwrap(); // blank in middle
            writeln!(f, "{good}").unwrap();
        }
        let records = read_last_n(&log_path, 10).expect("blank lines must be tolerated silently");
        assert_eq!(records.len(), 2, "2 good records, blanks ignored");
    }
}
