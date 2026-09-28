use std::process::Command;

#[test]
fn reporting_the_screen_recording_permission_prints_nothing_and_exits_with_zero_or_one() {
    let report = Command::new(env!("CARGO_BIN_EXE_yabai"))
        .arg("--report-screen-recording-permission")
        .output()
        .expect("the yabai binary built for this test run can be spawned");

    assert_eq!(String::from_utf8_lossy(&report.stdout), "");
    assert_eq!(String::from_utf8_lossy(&report.stderr), "");
    assert!(
        matches!(report.status.code(), Some(0 | 1)),
        "{:?}",
        report.status
    );
}
