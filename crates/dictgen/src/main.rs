//! `dictgen`: builds an offline dictionary file for the app.
//!
//! ```text
//! cargo run --release -p allreads-dictgen -- \
//!     --input kaikki.org-dictionary-French.jsonl.gz --language fr --output fr-en.sqlite
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

/// Builds an Allreads dictionary from a kaikki.org Wiktionary extract.
#[derive(Debug, Parser)]
#[command(version)]
struct Args {
    /// The extract, as JSON lines (`.jsonl` or `.jsonl.gz`).
    #[arg(long)]
    input: PathBuf,
    /// The language code to keep, e.g. `fr`. Its preset gives the term keys.
    #[arg(long)]
    language: String,
    /// Where to write the SQLite file (replaced if it exists).
    #[arg(long)]
    output: PathBuf,
    /// The pair stored in the file, e.g. `fr-en`. Defaults to `<language>-en`.
    #[arg(long)]
    pair: Option<String>,
    /// The build date stored in the file, e.g. `2026-11-25`.
    #[arg(long, default_value = "unknown")]
    built: String,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let pair = args.pair.unwrap_or_else(|| format!("{}-en", args.language));
    match allreads_dictgen::build(
        &args.input,
        &args.output,
        &args.language,
        &pair,
        &args.built,
    ) {
        Ok(counts) => {
            println!(
                "{}: {} entries, {} senses, {} forms",
                args.output.display(),
                counts.entries,
                counts.senses,
                counts.forms
            );
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("dictgen: {message}");
            ExitCode::FAILURE
        }
    }
}
