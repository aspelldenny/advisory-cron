//! Phase 1.1 acceptance: top-level `--help` exits 0 and lists all 5 subcommands.
//! Per-subcommand `<sub> --help` also exits 0 (clap derived).

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_advisory-cron");

#[test]
fn top_level_help_exits_zero_and_lists_all_subcommands() {
    let out = Command::new(BIN)
        .arg("--help")
        .output()
        .expect("failed to run binary");

    assert!(
        out.status.success(),
        "expected exit 0, got {:?}; stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr),
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    for sub in ["init", "register", "unregister", "run", "status"] {
        assert!(
            stdout.contains(sub),
            "expected --help output to mention `{sub}`, got:\n{stdout}"
        );
    }
}

#[test]
fn each_subcommand_help_exits_zero() {
    for sub in ["init", "register", "unregister", "run", "status"] {
        let out = Command::new(BIN)
            .args([sub, "--help"])
            .output()
            .unwrap_or_else(|e| panic!("failed to run `{sub} --help`: {e}"));

        assert!(
            out.status.success(),
            "subcommand `{sub} --help` failed: exit={:?} stderr={}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr),
        );
    }
}

#[test]
fn unknown_subcommand_exits_nonzero() {
    let out = Command::new(BIN)
        .arg("definitely-not-a-subcommand")
        .output()
        .expect("failed to run binary");

    assert!(
        !out.status.success(),
        "expected nonzero exit for unknown subcommand, got success"
    );
}
