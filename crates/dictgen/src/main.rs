//! `dictgen`: builds an offline dictionary file for the app.
//!
//! ```text
//! cargo run --release -p allreads-dictgen -- \
//!     --input kaikki.org-dictionary-French.jsonl.gz --language fr --output fr-en.sqlite \
//!     --frequencies Lexique383.tsv --frequency-source lexique
//! ```
//!
//! Without `--input`, the word frequencies are added to the dictionary at `--output`.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

/// Builds an Allreads dictionary from a kaikki.org Wiktionary extract.
#[derive(Debug, Parser)]
#[command(version)]
struct Args {
    /// The extract, as JSON lines (`.jsonl` or `.jsonl.gz`). Leave it out to
    /// add word frequencies to an existing dictionary.
    #[arg(long)]
    input: Option<PathBuf>,
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
    /// A word-frequency list for the vocabulary check.
    #[arg(long, requires = "frequency_source")]
    frequencies: Option<PathBuf>,
    /// Its format: `lexique` or `frequencywords`.
    #[arg(long, requires = "frequencies")]
    frequency_source: Option<String>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    if args.input.is_none() && args.frequencies.is_none() {
        eprintln!("dictgen: give --input to build a dictionary, --frequencies to add word frequencies, or both");
        return ExitCode::FAILURE;
    }
    if let Some(input) = &args.input {
        let pair = args
            .pair
            .clone()
            .unwrap_or_else(|| format!("{}-en", args.language));
        match allreads_dictgen::build(input, &args.output, &args.language, &pair, &args.built) {
            Ok(counts) => println!(
                "{}: {} entries, {} senses, {} forms",
                args.output.display(),
                counts.entries,
                counts.senses,
                counts.forms
            ),
            Err(message) => {
                eprintln!("dictgen: {message}");
                return ExitCode::FAILURE;
            }
        }
    }
    if let (Some(list), Some(name)) = (&args.frequencies, &args.frequency_source) {
        let Some(source) = allreads_dictgen::frequency::source_named(name) else {
            eprintln!("dictgen: unknown frequency source {name:?} (lexique, frequencywords)");
            return ExitCode::FAILURE;
        };
        match allreads_dictgen::frequency::add_frequencies(
            &args.output,
            source.as_ref(),
            list,
            &args.language,
        ) {
            Ok(summary) => println!(
                "{}: {} ranked words with {} forms from {} ({} left out)",
                args.output.display(),
                summary.ranked,
                summary.forms,
                source.name(),
                summary.skipped
            ),
            Err(message) => {
                eprintln!("dictgen: {message}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}
