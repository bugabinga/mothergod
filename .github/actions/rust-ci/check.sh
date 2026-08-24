#!/usr/bin/env bash
set -euo pipefail

checks=${1:?check set is required}
target=${2:-}
target_args=()
if [ -n "$target" ]; then
  target_args=(--target "$target")
fi

x_binary=x/target/debug/cargo-x
if [ -x "$x_binary.exe" ]; then
  x_binary="$x_binary.exe"
fi

run_fmt() {
  "$x_binary" fmt --check
}

run_clippy() {
  "$x_binary" lint
}

run_tests() {
  cargo test --all-targets "${target_args[@]}"
  if [ -z "$target" ]; then
    cargo test --manifest-path x/Cargo.toml
  fi
}

run_doctests() {
  cargo test --doc "${target_args[@]}"
}

run_rustdoc() {
  RUSTDOCFLAGS='--deny warnings' cargo doc --no-deps "${target_args[@]}"
}

case "$checks" in
  fmt)
    run_fmt
    ;;
  clippy)
    run_clippy
    ;;
  test)
    run_tests
    ;;
  doc)
    run_doctests
    run_rustdoc
    ;;
  runtime)
    run_tests
    run_doctests
    ;;
  canonical)
    run_fmt
    run_clippy
    run_tests
    run_doctests
    run_rustdoc
    ;;
  *)
    echo "unsupported check set: $checks" >&2
    exit 2
    ;;
esac
