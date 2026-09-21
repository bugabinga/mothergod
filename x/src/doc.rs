use std::path::Path;
use std::process::Command;

/// Builds crate documentation with rustdoc warnings denied, exactly the
/// CLAUDE.md doc gate: `RUSTDOCFLAGS="--deny warnings" cargo doc --no-deps`.
/// Then checks `docs/api-surface.txt` against what rustdoc just built
/// (issue #651): SIMPLICITY's public-API-surface half of the scorecard had
/// no publisher, and this is its source of truth, kept honest the same way
/// `bench/baseline.json` is -- a committed number the gate that can compute
/// it verifies on every run.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let status = Command::new("cargo")
        .args(["doc", "--no-deps"])
        .env("RUSTDOCFLAGS", "--deny warnings")
        .current_dir(root)
        .status()
        .map_err(|error| format!("cannot run `cargo doc --no-deps`: {error}"))?;

    if !status.success() {
        eprintln!("doc: documentation build failed");
        eprintln!("  next: RUSTDOCFLAGS=\"--deny warnings\" cargo doc --no-deps");
        return Ok(true);
    }

    if let Err(message) = check_api_surface(root) {
        eprintln!("doc: {message}");
        return Ok(true);
    }

    println!("doc: built without warnings");
    Ok(false)
}

/// Compares the item count rustdoc just built against the committed
/// `docs/api-surface.txt`. A mismatch means a PR changed the public API
/// without updating the ledger status-data.py publishes from; the fix is
/// naming the count this run measured.
fn check_api_surface(root: &Path) -> Result<(), String> {
    let name = package_name(root)?;
    let all_items_path = root.join("target/doc").join(&name).join("all.html");
    let html = std::fs::read_to_string(&all_items_path)
        .map_err(|error| format!("cannot read {}: {error}", all_items_path.display()))?;
    let built = count_all_items(&html);
    if built == 0 {
        return Err(format!(
            "{}: parsed 0 items; rustdoc's all.html shape may have changed",
            all_items_path.display()
        ));
    }

    let ledger_path = root.join("docs/api-surface.txt");
    let ledger_text = std::fs::read_to_string(&ledger_path)
        .map_err(|error| format!("cannot read {}: {error}", ledger_path.display()))?;
    let recorded: usize = ledger_text
        .trim()
        .parse()
        .map_err(|error| format!("{}: not an integer ({error})", ledger_path.display()))?;

    if built != recorded {
        return Err(format!(
            "public API surface is {built} items, but {} says {recorded}\n  next: printf '{built}\\n' > docs/api-surface.txt",
            ledger_path.display()
        ));
    }
    Ok(())
}

/// The root package's name from `[package] name = "..."` in `Cargo.toml`,
/// which is also rustdoc's output directory under `target/doc/`.
fn package_name(root: &Path) -> Result<String, String> {
    let path = root.join("Cargo.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut in_package = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[package]" {
            in_package = true;
            continue;
        }
        if !in_package {
            continue;
        }
        if trimmed.starts_with('[') {
            break;
        }
        let Some(value) = trimmed.strip_prefix("name") else {
            continue;
        };
        let Some(value) = value.trim_start().strip_prefix('=') else {
            continue;
        };
        let value = value.trim();
        if let Some(name) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
            return Ok(name.to_string());
        }
    }
    Err(format!(
        "{}: no [package] name = \"...\" found",
        path.display()
    ))
}

/// Every item rustdoc lists in `all.html`'s `<ul class="all-items">`
/// blocks, one per category (Enums, Functions, ...). The page's own "Crate
/// Items" sidebar table of contents wraps its links in `<li>` too, so a
/// bare count of every `<li>` in the file overcounts by one per category
/// present; restricting to these blocks excludes that nav.
fn count_all_items(html: &str) -> usize {
    const OPEN: &str = "<ul class=\"all-items\">";
    let mut count = 0;
    let mut rest = html;
    while let Some(start) = rest.find(OPEN) {
        let after_open = &rest[start + OPEN.len()..];
        let end = after_open.find("</ul>").unwrap_or(after_open.len());
        count += after_open[..end].matches("<li>").count();
        rest = &after_open[end..];
    }
    count
}

#[cfg(test)]
mod tests {
    use super::count_all_items;

    #[test]
    fn counts_items_in_all_items_blocks_not_the_sidebar_toc() {
        let page = concat!(
            r#"<nav class="sidebar"><ul class="block">"#,
            r##"<li><a href="#enums">Enums</a></li><li><a href="#functions">Functions</a></li>"##,
            "</ul></nav>",
            r#"<ul class="all-items"><li><a href="enum.Error.html">Error</a></li>"#,
            r#"<li><a href="enum.Method.html">Method</a></li></ul>"#,
            r#"<ul class="all-items"><li><a href="fn.compress.html">compress</a></li></ul>"#,
        );
        assert_eq!(count_all_items(page), 3);
    }

    #[test]
    fn a_page_with_no_all_items_block_counts_zero() {
        assert_eq!(
            count_all_items("<html><body>no items here</body></html>"),
            0
        );
    }
}
