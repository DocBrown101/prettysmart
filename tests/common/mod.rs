use std::process::Output;

#[track_caller]
pub fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "prettysmart failed: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
