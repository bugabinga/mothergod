//! The report binaries re-run themselves as `measure_all`'s child (issue
//! #1340), so each must serve child mode before it does anything else: one
//! that does not would run its whole report, fetch included, in the child.
//! Needs `corpus-fetch` because the binaries do.
#![cfg(feature = "corpus-fetch")]

use std::io::Write;
use std::process::{Command, Stdio};

fn round_trips_through_child_mode(program: &str) {
    let data = b"report binaries serve child mode first. ".repeat(100);
    let run = |mode: &str, input: &[u8]| {
        let mut process = Command::new(program)
            .args(["--child", mode])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("child spawns");
        process
            .stdin
            .take()
            .expect("piped")
            .write_all(input)
            .expect("small input");
        let output = process.wait_with_output().expect("child exits");
        assert!(output.status.success(), "{program} --child {mode}");
        output.stdout
    };
    let packed = run("compress", &data);
    assert!(packed.len() < data.len());
    assert_eq!(run("decompress", &packed), data);
}

#[test]
fn finals_report_serves_child_mode() {
    round_trips_through_child_mode(env!("CARGO_BIN_EXE_finals_report"));
}

#[test]
fn silesia_report_serves_child_mode() {
    round_trips_through_child_mode(env!("CARGO_BIN_EXE_silesia_report"));
}
