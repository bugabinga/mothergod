//! Allocation-failure torture sweep (#453, `docs/TESTING.md` layer 2's
//! "never overallocate" made concrete under real allocator failure, not
//! just a size ceiling): hard rule 2 says the decoder never panics on any
//! input, but nothing exercised decode under an allocation that actually
//! FAILS. curl's torture mode and SQLite's OOM injection are the same
//! mechanism independently invented: run a decode once to count its
//! allocator calls, then re-run it once per call with that one call
//! sabotaged, and assert the decoder still returns `Err`, never panics,
//! never aborts.
//!
//! Rust's infallible allocation paths (`Vec::push`, `vec![x; n]`, `to_vec`)
//! call `std::alloc::handle_alloc_error` on failure, which **aborts the
//! process**: no unwind, no `Err`, nothing this test's own `catch_unwind`
//! could observe from inside the same process. So each sabotaged call runs
//! in a fresh child process (`std::env::current_exe()` re-exec, gated by
//! `MOTHERGOD_TORTURE_CHILD`); the parent's only observation is the child's
//! exit status, which is exactly what distinguishes an abort (signalled,
//! never reaches this file's own `ExitCode::SUCCESS` return) from a graceful
//! `Err` (reached it).
//!
//! Harness-off by default (`harness = false` in `Cargo.toml`, this file is
//! `fn main`, not `#[test]`): spawning a child process per allocation call
//! is too slow for the required gate's fast-and-deterministic contract
//! (`docs/TESTING.md`'s doctrine table). Opt in with
//! `MOTHERGOD_TORTURE=1 cargo test --test torture -- --nocapture`; unset,
//! `cargo test --all-targets` still builds this binary but its `main` exits
//! immediately.
//!
//! Coverage today: `codec::decode`'s `output` buffer, the one allocation
//! whose size is attacker-controlled (`declared_len`, up to
//! `codec::MAX_DECODED_LEN`) rather than fixed by the model tables'
//! constant sizes — the "grows before validating" class hard rule 2 is
//! actually worried about, per the issue. `Models::new`'s fixed-size
//! tables and the `Delta`/`Bcj`/`Transpose` filters' undo buffers are
//! still infallible, so a sweep that reaches one of those allocations
//! still aborts; the summary this binary prints names exactly which
//! fixture/call index that is. Hardening those is real, separable
//! follow-up work (`fetch_add` through `Model::new`, `Literal::new`, and
//! each filter's `decode`, all currently `pub fn -> Self`/`-> Vec<u8>`
//! with no room for `Err`) — tracked on #453, not silently dropped.

use std::alloc::{GlobalAlloc, Layout, System};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, ExitStatus};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Ceiling on one sabotaged-call child's runtime. A graceful `Err` or an
/// abort both exit within milliseconds; the only way a child runs long is a
/// sabotaged allocation driving decode into a loop instead of failing fast,
/// exactly the case `Command::status()` alone would block on forever
/// (reviewer, PR #467).
const CHILD_TIMEOUT: Duration = Duration::from_secs(10);

/// A child's outcome once it either exits or is killed for outliving
/// [`CHILD_TIMEOUT`]. Not folded into [`ExitStatus`]: the standard library
/// gives no portable way to construct one for the timeout case.
enum ChildOutcome {
    Exited(ExitStatus),
    TimedOut,
}

impl ChildOutcome {
    fn is_graceful(&self) -> bool {
        matches!(self, Self::Exited(status) if status.success())
    }
}

impl std::fmt::Display for ChildOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exited(status) => write!(f, "exited {status}"),
            Self::TimedOut => write!(f, "timed out after {CHILD_TIMEOUT:?}, killed"),
        }
    }
}

/// Polls `child` for exit, killing it if it outlives `timeout`. Polling
/// rather than a blocking wait because the standard library has no portable
/// wait-with-timeout; the sweep runs at most a few hundred children, so the
/// poll interval's overhead is negligible next to spawn cost.
fn wait_with_timeout(mut child: Child, timeout: Duration) -> ChildOutcome {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("try_wait on child") {
            return ChildOutcome::Exited(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return ChildOutcome::TimedOut;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

struct TortureAlloc;

// Relaxed throughout: this binary never runs the counting/injection window
// (ARMED true) from more than one thread at a time, so there is nothing for
// a stronger ordering to synchronize with.
static ARMED: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicUsize = AtomicUsize::new(0);
static FAIL_AT: AtomicUsize = AtomicUsize::new(usize::MAX);

/// `None` outside the armed window (delegate as normal); `Some` inside it,
/// carrying whether this particular call is the one to fail. Centralizes
/// the count-and-compare so `alloc`/`alloc_zeroed`/`realloc` can't drift.
fn tick() -> bool {
    if !ARMED.load(Ordering::Relaxed) {
        return false;
    }
    let i = CALLS.fetch_add(1, Ordering::Relaxed);
    i == FAIL_AT.load(Ordering::Relaxed)
}

// SAFETY: every arm either returns a pointer straight from `System`, whose
// `GlobalAlloc` impl is trusted, or `ptr::null_mut()`, `GlobalAlloc`'s own
// documented spelling for "allocation failed". `dealloc` always forwards:
// freeing a pointer this allocator never actually withheld (the null case
// is never passed back to `dealloc`, since Rust's allocation failure paths
// never call `dealloc` on a null they got from `alloc`) must still work.
unsafe impl GlobalAlloc for TortureAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if tick() {
            return std::ptr::null_mut();
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if tick() {
            return std::ptr::null_mut();
        }
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if tick() {
            return std::ptr::null_mut();
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOC: TortureAlloc = TortureAlloc;

/// One fixture to sweep, named `<kind>:<file name>` so a child process can
/// resolve the same path the parent counted allocations against, without
/// the parent having to pass a full path through argv or env (`Command`
/// already inherits `MOTHERGOD_GOLDEN_DIR`/`MOTHERGOD_ADVERSARIAL_DIR`, the
/// same overrides `tests/golden.rs`/`tests/adversarial.rs` read, for the
/// Android runner's benefit).
struct Fixture {
    id: String,
    path: PathBuf,
}

fn golden_dir() -> PathBuf {
    env::var_os("MOTHERGOD_GOLDEN_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden"),
        PathBuf::from,
    )
}

fn adversarial_dir() -> PathBuf {
    env::var_os("MOTHERGOD_ADVERSARIAL_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/adversarial"),
        PathBuf::from,
    )
}

fn fixtures() -> Vec<Fixture> {
    let mut out = Vec::new();
    for entry in fs::read_dir(golden_dir()).expect("tests/golden must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.extension().is_some_and(|ext| ext == "mgdc") {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap()
                .to_owned();
            out.push(Fixture {
                id: format!("golden:{name}"),
                path,
            });
        }
    }
    for entry in fs::read_dir(adversarial_dir()).expect("tests/adversarial must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.is_file() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap()
                .to_owned();
            out.push(Fixture {
                id: format!("adversarial:{name}"),
                path,
            });
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

fn fixture_path(id: &str) -> PathBuf {
    let (kind, name) = id.split_once(':').expect("fixture id is kind:name");
    match kind {
        "golden" => golden_dir().join(name),
        "adversarial" => adversarial_dir().join(name),
        other => panic!("unknown fixture kind {other:?} in id {id:?}"),
    }
}

/// Runs one decode with the allocator armed. `fail_at = usize::MAX` (never
/// matched by `CALLS`, which starts at 0 and only grows) is the counting
/// mode: nothing fails, and the return value is the total call count.
fn armed_decode(data: &[u8], fail_at: usize) -> usize {
    FAIL_AT.store(fail_at, Ordering::Relaxed);
    CALLS.store(0, Ordering::Relaxed);
    ARMED.store(true, Ordering::Relaxed);
    let _ = mothergod::decompress(data);
    ARMED.store(false, Ordering::Relaxed);
    CALLS.load(Ordering::Relaxed)
}

/// Child mode: decode once with the allocator sabotaged at one specific
/// call, then exit success. Reaching the `ExitCode::SUCCESS` below is the
/// entire assertion — an abort inside `armed_decode` kills this process
/// before that, which is what the parent's exit-status check catches.
fn run_child(spec: &str) -> ExitCode {
    // rsplit_once: `id` is itself `kind:name` (fixture_path's format), so
    // the boundary this spec cares about is the last colon, not the first.
    let (id, fail_at) = spec.rsplit_once(':').expect("child spec is id:fail_at");
    let fail_at: usize = fail_at.parse().expect("fail_at is a usize");
    let data = fs::read(fixture_path(id)).unwrap_or_else(|e| panic!("read fixture {id:?}: {e}"));
    armed_decode(&data, fail_at);
    ExitCode::SUCCESS
}

fn run_sweep() -> ExitCode {
    let self_exe = env::current_exe().expect("current_exe must resolve for re-exec");
    let mut failures = Vec::new();
    let mut total_calls = 0usize;
    let mut swept = 0usize;

    for fixture in fixtures() {
        swept += 1;
        let data = fs::read(&fixture.path)
            .unwrap_or_else(|e| panic!("read {}: {e}", fixture.path.display()));
        let n = armed_decode(&data, usize::MAX);
        total_calls += n;
        for k in 0..n {
            let child = Command::new(&self_exe)
                .env("MOTHERGOD_TORTURE_CHILD", format!("{}:{k}", fixture.id))
                .spawn()
                .unwrap_or_else(|e| panic!("spawn child for {}:{k}: {e}", fixture.id));
            let outcome = wait_with_timeout(child, CHILD_TIMEOUT);
            if !outcome.is_graceful() {
                failures.push((fixture.id.clone(), k, n, outcome));
            }
        }
        println!("torture: {} — {n} allocator calls swept", fixture.id);
    }

    if failures.is_empty() {
        println!("torture: {total_calls} allocator calls swept clean across {swept} fixtures");
        return ExitCode::SUCCESS;
    }

    println!(
        "torture: {} sabotaged call(s) did not return gracefully:",
        failures.len()
    );
    for (id, k, n, outcome) in &failures {
        println!("  {id}: call {k} of {n} — child {outcome}");
    }
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    if let Ok(spec) = env::var("MOTHERGOD_TORTURE_CHILD") {
        return run_child(&spec);
    }
    if env::var_os("MOTHERGOD_TORTURE").is_none() {
        println!("torture: skipped (set MOTHERGOD_TORTURE=1 to run the allocation-failure sweep)");
        return ExitCode::SUCCESS;
    }
    run_sweep()
}
