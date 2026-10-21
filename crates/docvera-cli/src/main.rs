const GUIDE: &str = concat!(
    include_str!("../../../docs/cli-guide.md"),
    "\n---\n\n",
    include_str!("../../../docs/schema.md"),
    "\n---\n\n",
    include_str!("../../../docs/shacl.md"),
);

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use docvera_engine::{
    Workspace,
    index::{self, SearchOptions},
};
use serde_json::{Value, json};
use std::{
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

#[derive(Parser)]
#[command(
    name = "docvera",
    version,
    about = "Local-first Markdown schema validator and knowledge engine",
    after_help = "Run `docvera guide` for the complete offline manual with examples.\nFor AI agents: `docvera guide --format json` includes the manual and command contracts."
)]
struct Cli {
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,
    #[arg(long, global = true, value_enum, default_value = "human")]
    format: Format,
    #[command(subcommand)]
    command: Command,
}
#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Human,
    Json,
    Jsonl,
}
#[derive(Subcommand)]
enum Command {
    /// Create a runnable three-document workspace; never overwrite files.
    Init,
    /// Validate source files without requiring an index.
    Check {
        #[arg(long)]
        warnings_as_errors: bool,
    },
    /// Refresh the disposable index (also records validation diagnostics).
    Index {
        #[arg(long)]
        rebuild: bool,
    },
    /// Query the last explicitly indexed snapshot.
    Query {
        #[command(subcommand)]
        query: Query,
    },
    /// Traverse resolved document references; cycles terminate.
    Graph {
        #[arg(value_enum)]
        direction: Direction,
        id: String,
        #[arg(long, default_value_t = 1)]
        depth: usize,
    },
    /// Literal-phrase full-text search with BM25 ranking.
    Search {
        text: String,
        #[arg(long = "type")]
        document_type: Option<String>,
        #[arg(long)]
        section: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Retrieve original Markdown for a section ID from the indexed snapshot.
    Extract { section_id: String },
    /// Print the complete offline usage guide with examples (JSON for agents).
    Guide,
    /// Print a generated JSON Schema (no workspace required).
    Schema {
        #[arg(value_enum)]
        kind: SchemaKind,
    },
    /// Print versioned machine-readable command and result contracts.
    Contract,
}
#[derive(Subcommand)]
enum Query {
    Sql { sql: String },
}
#[derive(Clone, Copy, ValueEnum)]
enum SchemaKind {
    Config,
    Document,
    CheckResult,
    Diagnostic,
    IndexResult,
}

/// Prose command metadata stays explicit; data schemas are always generated.
fn contract() -> Result<Value> {
    let mut descriptor: Value =
        serde_json::from_str(include_str!("../../../docs/contracts/v1.json"))?;
    let schemas = serde_json::to_value(docvera_engine::contracts::json_schemas())?;
    // Keep the descriptor's compact field lists in sync too, not just its full schemas.
    descriptor["results"]["check"] = schemas["check-result"]["required"].clone();
    descriptor["results"]["index"] = schemas["index-result"]["required"].clone();
    descriptor["diagnostic"] = schemas["diagnostic"]["required"].clone();
    descriptor["jsonSchemas"] = schemas;
    Ok(descriptor)
}

#[derive(Clone, ValueEnum)]
enum Direction {
    Incoming,
    Outgoing,
}

fn run(cli: &Cli) -> Result<(Value, u8)> {
    let root = &cli.root;
    Ok(match &cli.command {
        Command::Init => (
            json!({"contractVersion":1,"created":docvera_engine::init(root)?}),
            0,
        ),
        Command::Check { warnings_as_errors } => {
            let result = Workspace::load(root)?.check();
            let invalid = !result.valid()
                || (*warnings_as_errors
                    && result.diagnostics.iter().any(|d| d.severity == "warning"));
            (serde_json::to_value(result)?, u8::from(invalid))
        }
        Command::Index { rebuild } => (
            serde_json::to_value(index::index(&Workspace::load(root)?, *rebuild)?)?,
            0,
        ),
        Command::Query {
            query: Query::Sql { sql },
        } => (index::query(root, sql)?, 0),
        Command::Graph {
            direction,
            id,
            depth,
        } => (
            index::graph(
                root,
                id,
                match direction {
                    Direction::Incoming => "incoming",
                    Direction::Outgoing => "outgoing",
                },
                *depth,
            )?,
            0,
        ),
        Command::Search {
            text,
            document_type,
            section,
            limit,
        } => (
            index::search(
                root,
                &SearchOptions {
                    text: text.clone(),
                    document_type: document_type.clone(),
                    section: section.clone(),
                    limit: Some(*limit),
                },
            )?,
            0,
        ),
        Command::Extract { section_id } => (index::extract(root, section_id)?, 0),
        Command::Guide => (
            json!({
                "contractVersion": 1,
                "guide": GUIDE,
                "contract": contract()?,
            }),
            0,
        ),
        Command::Schema { kind } => {
            use docvera_engine::contracts;
            let schema = match kind {
                SchemaKind::Config => contracts::config_schema(),
                SchemaKind::Document => contracts::document_schema(),
                SchemaKind::CheckResult => contracts::check_result_schema(),
                SchemaKind::Diagnostic => contracts::diagnostic_schema(),
                SchemaKind::IndexResult => contracts::index_result_schema(),
            };
            (serde_json::to_value(schema)?, 0)
        }
        Command::Contract => (contract()?, 0),
    })
}
fn emit(value: &Value, format: Format) -> Result<()> {
    let mut out = io::stdout().lock();
    match format {
        Format::Json => writeln!(out, "{}", serde_json::to_string_pretty(value)?)?,
        Format::Jsonl => writeln!(out, "{}", serde_json::to_string(value)?)?,
        Format::Human => {
            if let Some(guide) = value.get("guide").and_then(Value::as_str) {
                write!(out, "{guide}")?;
            } else if let Some(diagnostics) = value.get("diagnostics").and_then(Value::as_array) {
                for d in diagnostics {
                    writeln!(
                        out,
                        "{}:{}:{}: {} [{}] {}",
                        d["path"].as_str().unwrap_or(""),
                        d["span"]["line"],
                        d["span"]["column"],
                        d["severity"].as_str().unwrap_or(""),
                        d["rule"].as_str().unwrap_or(""),
                        d["message"].as_str().unwrap_or("")
                    )?;
                }
                writeln!(
                    out,
                    "Checked {} documents; {} diagnostics",
                    value["documents"],
                    diagnostics.len()
                )?;
            } else {
                writeln!(out, "{}", serde_json::to_string_pretty(value)?)?;
            }
        }
    }
    Ok(())
}
fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok((value, code)) => match emit(&value, cli.format) {
            Ok(()) => ExitCode::from(code),
            Err(error) => {
                eprintln!("docvera: {error:#}");
                ExitCode::from(2)
            }
        },
        Err(error) => {
            let value = json!({"contractVersion":1,"error":{"code":"operation_failed","message":format!("{error:#}")}});
            match cli.format {
                Format::Human => eprintln!("docvera: {error:#}"),
                _ => {
                    if let Err(e) = emit(&value, cli.format) {
                        eprintln!("docvera: {e}");
                    }
                }
            }
            ExitCode::from(2)
        }
    }
}
