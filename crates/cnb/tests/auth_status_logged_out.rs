//! `cnb auth status` with no credentials → exit code 4 + helpful hint.

mod common;

use predicates::prelude::*;

#[test]
fn auth_status_when_logged_out_exits_4() {
    let env = common::TestEnv::new();
    let assert = env.cmd().args(["auth", "status"]).assert();
    let output = assert.get_output().clone();
    eprintln!("STDOUT: {}", String::from_utf8_lossy(&output.stdout));
    eprintln!("STDERR: {}", String::from_utf8_lossy(&output.stderr));
    assert
        .failure()
        .code(4)
        .stderr(predicate::str::contains("cnb auth login"));
}
