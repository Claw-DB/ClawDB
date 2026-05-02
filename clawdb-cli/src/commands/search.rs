//! `clawdb search` — GET /v1/memories/search for semantic or FTS search.

use clap::Args;
use tabled::Tabled;

use crate::client::ClawDBClient;
use crate::error::CliResult;
use crate::output::{self, OutputFormat};
use crate::types::SearchHit;

#[derive(Debug, Clone, Args)]
pub struct SearchArgs {
    /// Search query.
    pub query: String,

    /// Maximum number of results.
    #[arg(long, default_value_t = 10)]
    pub top_k: u32,

    /// Use semantic (vector) search. Pass --no-semantic for FTS only.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub semantic: bool,

    /// Filter expression as a JSON string.
    #[arg(long)]
    pub filter: Option<String>,
}

#[derive(Tabled, Clone)]
struct SearchRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Content Preview")]
    content: String,
    #[tabled(rename = "Score")]
    score: String,
    #[tabled(rename = "Tags")]
    tags: String,
}

pub async fn execute(
    args: SearchArgs,
    client: &ClawDBClient,
    fmt: &OutputFormat,
    quiet: bool,
) -> CliResult<()> {
    let top_k = args.top_k.to_string();
    let semantic = args.semantic.to_string();
    let mut params: Vec<(&str, &str)> = vec![
        ("q", &args.query),
        ("top_k", &top_k),
        ("semantic", &semantic),
    ];
    let filter_owned;
    if let Some(f) = &args.filter {
        filter_owned = f.clone();
        params.push(("filter", &filter_owned));
    }

    let hits: Vec<SearchHit> = client.get_q("/v1/memories/search", &params).await?;

    match output::effective_format(fmt) {
        OutputFormat::Json => output::print_json(&hits, quiet),
        OutputFormat::Tsv => {
            let rows = to_rows(&hits);
            output::print_tsv(&rows, quiet);
        }
        OutputFormat::Table => {
            let rows = to_rows(&hits);
            output::print_table(&rows, quiet);
        }
    }

    Ok(())
}

fn to_rows(hits: &[SearchHit]) -> Vec<SearchRow> {
    hits.iter()
        .map(|h| SearchRow {
            id: truncate(&h.id, 12),
            content: truncate(&h.content, 80),
            score: format!("{:.4}", h.score),
            tags: h.tags.join(", "),
        })
        .collect()
}

fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > max {
        format!("{}…", chars[..max].iter().collect::<String>())
    } else {
        s.to_string()
    }
}
