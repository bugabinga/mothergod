#!/usr/bin/env bash
set -euo pipefail

checks=${1:?check set is required}
target=${2:-}

x_binary=x/target/debug/cargo-x
if [ -x "$x_binary.exe" ]; then
  x_binary="$x_binary.exe"
fi

# fmt, clippy, test, doc, and canonical are stages of x's quality gate;
# x owns their command lists (ADR-0029, issue #227) and the gate is
# native-only by design. runtime is monster's cross-triple execution
# sweep, target-parameterized by nature, so its commands live here.
require_native() {
  if [ -n "$target" ]; then
    echo "check set '$checks' delegates to x, which takes no target; use 'runtime'" >&2
    exit 2
  fi
}

case "$checks" in
  fmt)
    require_native
    "$x_binary" fmt --check
    ;;
  clippy)
    require_native
    "$x_binary" lint
    ;;
  test)
    require_native
    "$x_binary" test
    ;;
  doc)
    require_native
    "$x_binary" doc
    ;;
  canonical)
    require_native
    "$x_binary" check
    ;;
  runtime)
    if [ -z "$target" ]; then
      echo "check set 'runtime' requires a target; the native plan is 'test'" >&2
      exit 2
    fi
    # mothergod-bench excluded: its reference.rs module shells out to the
    # host's zstd/xz/gzip binaries (research/corpus/POLICY.md), which
    # verify nothing about mothergod's own cross-compiled runtime and
    # don't exist on-device in Android's Bionic shell (issue #618).
    # `cargo x test` already runs the crate natively, every PR.
    cargo test --workspace --exclude mothergod-bench --all-targets --target "$target"
    cargo test --workspace --exclude mothergod-bench --doc --target "$target"
    ;;
  *)
    echo "unsupported check set: $checks" >&2
    exit 2
    ;;
esac
