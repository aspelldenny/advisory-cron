//! Phase 3.2 — Linux crontab scheduler.
//!
//! P012 ships this as STUB returning `bail!` for every method.
//! Real implementation lands in P013 (Phase 3.2):
//!   - `register`: `crontab -l` → parse → append `<cron_expr> <self_exe> run --config <path> # advisory-cron: <label>` → `crontab -` pipe back
//!   - `unregister`: same flow, filter out tag line
//!   - `status`: grep tag from `crontab -l`

use anyhow::{Result, bail};

use super::{RegisterIntent, RegisterReport, Scheduler, SchedulerStatus, UnregisterReport};

#[derive(Debug, Default)]
pub struct CrontabScheduler;

impl Scheduler for CrontabScheduler {
    fn register(&self, _intent: &RegisterIntent) -> Result<RegisterReport> {
        bail!("CrontabScheduler::register — Phase 3.2 (P013) chưa ship")
    }

    fn unregister(&self, _label: &str) -> Result<UnregisterReport> {
        bail!("CrontabScheduler::unregister — Phase 3.2 (P013) chưa ship")
    }

    fn status(&self, _label: &str) -> Result<SchedulerStatus> {
        bail!("CrontabScheduler::status — Phase 3.2 (P013) chưa ship")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_register_bails_with_p013_message() {
        let s = CrontabScheduler;
        let intent = RegisterIntent {
            label: "test".into(),
            hour: 9,
            minute: 0,
            self_exe: std::path::PathBuf::from("/bin/x"),
            working_dir: std::path::PathBuf::from("/tmp"),
        };
        let err = s.register(&intent).unwrap_err();
        assert!(format!("{err:#}").contains("P013"));
    }

    #[test]
    fn stub_unregister_bails_with_p013_message() {
        let s = CrontabScheduler;
        let err = s.unregister("test").unwrap_err();
        assert!(format!("{err:#}").contains("P013"));
    }

    #[test]
    fn stub_status_bails_with_p013_message() {
        let s = CrontabScheduler;
        let err = s.status("test").unwrap_err();
        assert!(format!("{err:#}").contains("P013"));
    }
}
