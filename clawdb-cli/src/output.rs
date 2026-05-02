use std::fmt;

use colored::Colorize;
use serde::Serialize;
use tabled::{Table, Tabled};

/// Output format selector, parsed from --output flag.
#[derive(Debug, Clone, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// Pretty-printed table (default for TTY).
    #[default]
    Table,
    /// JSON output (default when stdout is not a TTY).
    Json,
    /// Tab-separated values.
    Tsv,
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Tsv => write!(f, "tsv"),
        }
    }
}

/// Returns the effective format, defaulting to Json when stdout is not a TTY.
pub fn effective_format(requested: &OutputFormat) -> OutputFormat {
    match requested {
        OutputFormat::Table => {
            use std::io::IsTerminal;
            if std::io::stdout().is_terminal() {
                OutputFormat::Table
            } else {
                OutputFormat::Json
            }
        }
        other => other.clone(),
    }
}

pub fn print_table<T: Tabled + Clone>(rows: &[T], quiet: bool) {
    if quiet {
        return;
    }
    println!("{}", Table::new(rows.iter().cloned()));
}

pub fn print_json<T: Serialize>(value: &T, quiet: bool) {
    if quiet {
        return;
    }
    match serde_json::to_string_pretty(value) {
        Ok(s) => println!("{}", s),
        Err(e) => eprintln!("{} serialization error: {}", "✗".red(), e),
    }
}

pub fn print_tsv<T: Tabled>(rows: &[T], quiet: bool) {
    if quiet || rows.is_empty() {
        return;
    }
    let headers = T::headers();
    println!(
        "{}",
        headers
            .iter()
            .map(|h| h.as_ref())
            .collect::<Vec<_>>()
            .join("\t")
    );
    for row in rows {
        let fields = row.fields();
        println!(
            "{}",
            fields
                .iter()
                .map(|f| f.as_ref())
                .collect::<Vec<_>>()
                .join("\t")
        );
    }
}

pub fn print_success(msg: &str, fmt: &OutputFormat, quiet: bool) {
    if quiet {
        return;
    }
    match fmt {
        OutputFormat::Json => println!("{}", serde_json::json!({"ok": true, "message": msg})),
        _ => println!("{} {}", "✓".green(), msg),
    }
}

pub fn print_error(msg: &str, fmt: &OutputFormat) {
    match fmt {
        OutputFormat::Json => eprintln!("{}", serde_json::json!({"error": msg})),
        _ => eprintln!("{} {}", "✗".red(), msg),
    }
}

pub fn print_warning(msg: &str, fmt: &OutputFormat, quiet: bool) {
    if quiet {
        return;
    }
    match fmt {
        OutputFormat::Json => {} // suppress warnings in JSON mode
        _ => eprintln!("{} {}", "⚠".yellow(), msg),
    }
}
