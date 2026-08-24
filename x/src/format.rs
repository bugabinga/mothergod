use std::borrow::Cow;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use dprint_plugin_typescript::configuration::ConfigurationBuilder;
use dprint_plugin_typescript::{FormatTextOptions, format_text as format_script};
use markup_fmt::{Language, config::FormatOptions as MarkupOptions, format_text as format_markup};
use pretty_yaml::{config::FormatOptions as YamlOptions, format_text as format_yaml};
use serde_json::Value;
use taplo::formatter::{Options as TomlOptions, format as format_toml};
use taplo::parser::parse as parse_toml;
use tempfile::NamedTempFile;

use crate::files::{FileKind, Selection, kind};

pub(crate) fn run(selection: &Selection, check: bool) -> Result<bool, String> {
    let mut rust_files = Vec::new();
    let mut findings = 0;
    let mut changed = 0;

    for relative in &selection.files {
        if kind(relative) == Some(FileKind::Rust) {
            rust_files.push(relative.clone());
            continue;
        }

        let path = selection.root.join(relative);
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("{}: cannot read UTF-8 text: {error}", relative.display()))?;
        match format_document(relative, &source) {
            Ok(formatted) if formatted == source => {}
            Ok(_) if check => {
                findings += 1;
                eprintln!("{}: needs formatting", relative.display());
                eprintln!("  fix: cargo x fmt -- {}", relative.display());
            }
            Ok(formatted) => {
                replace(&path, formatted.as_bytes())?;
                changed += 1;
            }
            Err(error) => {
                findings += 1;
                eprintln!("{}: cannot format: {error}", relative.display());
                eprintln!(
                    "  next: fix the syntax, then run `cargo x fmt -- {}`",
                    relative.display()
                );
            }
        }
    }

    if !rust_files.is_empty() && !run_rustfmt(&selection.root, &rust_files, check)? {
        findings += 1;
        eprintln!("Rust formatting failed");
        eprintln!("  fix: cargo x fmt -- {}", display_paths(&rust_files));
    }

    if findings == 0 {
        if check {
            println!("fmt: {} files checked", selection.files.len());
        } else if changed == 0 {
            println!("fmt: {} files already formatted", selection.files.len());
        } else {
            println!("fmt: {changed} files formatted");
        }
    } else {
        eprintln!("fmt: {findings} finding(s)");
    }

    Ok(findings > 0)
}

pub(crate) fn replace(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{}: has no parent directory", path.display()))?;
    let permissions = fs::metadata(path)
        .map_err(|error| format!("{}: cannot read metadata: {error}", path.display()))?
        .permissions();
    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|error| format!("{}: cannot create temporary file: {error}", path.display()))?;
    temporary
        .write_all(contents)
        .and_then(|()| temporary.flush())
        .map_err(|error| format!("{}: cannot write formatted file: {error}", path.display()))?;
    temporary
        .as_file()
        .set_permissions(permissions)
        .map_err(|error| format!("{}: cannot preserve permissions: {error}", path.display()))?;
    temporary.persist(path).map_err(|error| {
        format!(
            "{}: cannot replace file atomically: {}",
            path.display(),
            error.error
        )
    })?;
    Ok(())
}

fn format_document(path: &Path, source: &str) -> Result<String, String> {
    let formatted = match kind(path) {
        Some(FileKind::Json) => format_json(source)?,
        Some(FileKind::JsonLines) => format_json_lines(source)?,
        Some(FileKind::Toml) => format_toml_document(source)?,
        Some(FileKind::Yaml) => {
            format_yaml(source, &YamlOptions::default()).map_err(|error| error.to_string())?
        }
        Some(FileKind::Script) => {
            let config = ConfigurationBuilder::new().build();
            format_script(FormatTextOptions {
                path,
                extension: None,
                text: source.to_owned(),
                config: &config,
                external_formatter: None,
            })
            .map_err(|error| error.to_string())?
            .unwrap_or_else(|| source.to_owned())
        }
        Some(FileKind::Markup) => {
            let language = if path.extension().is_some_and(|extension| extension == "svg") {
                Language::Xml
            } else {
                Language::Html
            };
            format_markup(source, language, &MarkupOptions::default(), |code, _| {
                Ok(Cow::Borrowed(code))
            })
            .map_err(|error| error.to_string())?
        }
        Some(FileKind::Rust | FileKind::Markdown) | None => {
            return Err("internal error: formatter received an unsupported file".into());
        }
    };
    Ok(with_final_newline(formatted))
}

fn format_json(source: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(source).map_err(|error| {
        format!(
            "JSON syntax error at line {}, column {}: {error}",
            error.line(),
            error.column()
        )
    })?;
    serde_json::to_string_pretty(&value).map_err(|error| error.to_string())
}

fn format_json_lines(source: &str) -> Result<String, String> {
    let mut records = 0;
    for (index, line) in source.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        serde_json::from_str::<Value>(line)
            .map_err(|error| format!("JSONL syntax error on line {}: {error}", index + 1))?;
        records += 1;
    }
    if records == 0 && !source.is_empty() {
        return Err("JSONL contains no records".into());
    }
    Ok(source.to_owned())
}

fn format_toml_document(source: &str) -> Result<String, String> {
    let parsed = parse_toml(source);
    if !parsed.errors.is_empty() {
        let errors = parsed
            .errors
            .iter()
            .map(|error| {
                let offset = u32::from(error.range.start()) as usize;
                let (line, column) = line_column(source, offset);
                format!("line {line}, column {column}: {}", error.message)
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!("TOML syntax error: {errors}"));
    }
    Ok(format_toml(source, TomlOptions::default()))
}

fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let prefix = &source[..offset.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix, |(_, tail)| tail)
        .chars()
        .count()
        + 1;
    (line, column)
}

fn with_final_newline(mut text: String) -> String {
    if text.is_empty() {
        return text;
    }
    while text.ends_with(['\n', '\r']) {
        text.pop();
    }
    text.push('\n');
    text
}

fn run_rustfmt(root: &Path, files: &[PathBuf], check: bool) -> Result<bool, String> {
    let mut command = Command::new("rustfmt");
    command
        .current_dir(root)
        .args(["--edition", "2024", "--config", "skip_children=true"]);
    if check {
        command.arg("--check");
    }
    command.arg("--").args(files);
    command
        .status()
        .map(|status| status.success())
        .map_err(|error| {
            format!("cannot run rustfmt: {error}; install it with `rustup component add rustfmt`")
        })
}

fn display_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_formatters_are_idempotent() {
        let cases = [
            ("a.json", "{\"a\":1}"),
            ("a.jsonl", "{\"a\": 1}\n{\"b\":2}"),
            ("a.toml", "[a]\nx=1"),
            ("a.yml", "a:   1"),
            ("a.js", "const  x=1;"),
            ("a.html", "<main><p>x</p></main>"),
            ("a.svg", "<svg><path d=\"M0 0\"/></svg>"),
        ];

        for (path, source) in cases {
            let once = format_document(Path::new(path), source).unwrap();
            let twice = format_document(Path::new(path), &once).unwrap();
            assert_eq!(once, twice, "{path}");
        }
        assert_eq!(format_document(Path::new("empty.jsonl"), "").unwrap(), "");
    }

    #[test]
    fn structured_formatters_reject_malformed_input() {
        let cases = [
            ("a.json", "{"),
            ("a.jsonl", "{}\n{"),
            ("a.toml", "["),
            ("a.yml", "{"),
            ("a.js", "const = ;"),
            ("a.html", "<div></span>"),
            ("a.svg", "<svg><"),
        ];

        for (path, source) in cases {
            assert!(format_document(Path::new(path), source).is_err(), "{path}");
        }
    }
}
