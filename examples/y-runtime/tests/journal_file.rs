//! The journal kept in a file, `Y_JOURNAL`: what a workflow records there is read back by a run that starts afresh. In
//! a test binary of its own, as the variable is the process's.
use y_runtime::journal::{records, step, workflow};

#[test]
fn a_workflow_recorded_in_a_file_finishes_once() {
    let path = std::env::temp_dir().join(format!("y-journal-test-{}", std::process::id()));
    let _ = std::fs::remove_file(&path);
    std::env::set_var("Y_JOURNAL", &path);
    let mut made = 0;
    let mut run = || workflow("m::w", "b1@s1", vec!["7".into()], || step("submit", || { made += 1; 40u32 }) + 2);
    assert_eq!(run(), 42);
    assert_eq!(run(), 42);
    drop(run);
    assert_eq!(made, 1, "a finished workflow gives its recorded result");
    // the records are lines of the file: identity, version, step, what, value
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text, "m::w(7)\tb1@s1\t1\tsubmit\t40\nm::w(7)\tb1@s1\t0\tresult\t42\n");
    assert_eq!(records().len(), 2);
    let _ = std::fs::remove_file(&path);
}
