//! Minimal JSON string encode/decode: enough to round-trip text this
//! project itself writes, not a general JSON library — the same
//! deliberate scope limit `bench::baseline`'s hand-rolled reader takes
//! (`bench/src/baseline.rs`), taken here because `site/status-data.json`
//! only ever needs to carry this crate's own strings.

use std::fmt::Write as _;

/// Encodes `raw` as the body of a JSON string (the text between the
/// quotes `render` adds around it).
#[must_use]
pub fn escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for c in raw.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                write!(out, "\\u{:04x}", c as u32).expect("writing to a String never fails");
            }
            c => out.push(c),
        }
    }
    out
}

/// Decodes a JSON string body (the text `experiments::field` extracted
/// between a field's quotes) back to plain text. Handles the
/// escapes `research/progress.jsonl`'s own writers plausibly produce
/// (`\"`, `\\`, `\/`, `\n`, `\t`, `\r`); any other backslash sequence
/// (e.g. a `\uXXXX` this project has never written) passes through
/// unchanged rather than erroring, since this reader's only job is
/// round-tripping what this project itself writes, not validating
/// arbitrary JSON.
#[must_use]
pub fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('"') => out.push('"'),
            Some('/') => out.push('/'),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') | None => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_handles_quotes_and_backslashes() {
        assert_eq!(escape(r#"a "quoted" \path\"#), r#"a \"quoted\" \\path\\"#);
    }

    #[test]
    fn escape_handles_control_characters() {
        assert_eq!(escape("line1\nline2\ttab"), "line1\\nline2\\ttab");
        assert_eq!(escape("\u{1}"), "\\u0001");
    }

    #[test]
    fn unescape_reverses_escape() {
        let raw = "a \"quoted\" \\path\\, line\nbreak\ttab";
        assert_eq!(unescape(&escape(raw)), raw);
    }

    #[test]
    fn unescape_passes_through_unknown_escapes() {
        assert_eq!(unescape(r"\q"), r"\q");
    }

    #[test]
    fn unescape_handles_trailing_backslash() {
        assert_eq!(unescape(r"trailing\"), r"trailing\");
    }
}
