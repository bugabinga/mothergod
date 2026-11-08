//! `decode_profile`'s argument guard (issue #1935): a flag, or no argument,
//! prints the usage line and exits 2 before any `perf` check or build.

use std::process::Command;

fn run(args: &[&str]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_decode_profile"))
        .args(args)
        .output()
        .expect("decode_profile spawns");
    (
        output.status.code().expect("exits, not killed"),
        String::from_utf8(output.stderr).expect("utf8"),
    )
}

#[test]
#[cfg_attr(
    target_os = "android",
    ignore = "android-runner ships only the test executable; CARGO_BIN_EXE_decode_profile is a host path (#518)"
)]
fn a_flag_or_no_argument_prints_the_usage_and_exits_two() {
    for args in [
        &[][..],
        &["--help"],
        &["-x"],
        &["-"],
        &["file.txt", "--help"],
    ] {
        assert_eq!(
            run(args),
            (2, "usage: decode_profile <file>...\n".to_owned()),
            "{args:?}"
        );
    }
}
