//! `catalog`: builds a book catalog for the app's "Find a book".
//!
//! ```text
//! cargo run --release -p allreads-dictgen --bin catalog -- \
//!     --list books.jsonl --books epubs/ --dictionary fr-en.sqlite \
//!     --language fr --output fr-books.sqlite
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

/// Builds an Allreads book catalog from EPUB files and a list describing them.
#[derive(Debug, Parser)]
#[command(version)]
struct Args {
    /// The books, one JSON object per line (id, title, author, subjects, url, popularity).
    #[arg(long)]
    list: PathBuf,
    /// The folder with the books as `<id>.epub`.
    #[arg(long)]
    books: PathBuf,
    /// The offline dictionary to count with, as the app does once it is installed.
    #[arg(long)]
    dictionary: Option<PathBuf>,
    /// The books' language code, e.g. `fr`.
    #[arg(long)]
    language: String,
    /// Where to write the catalog (must not exist yet).
    #[arg(long)]
    output: PathBuf,
    /// Words met fewer times in a book are kept only in its total, as new words.
    #[arg(long, default_value_t = 2)]
    min_count: u32,
}

fn main() -> ExitCode {
    let args = Args::parse();
    match allreads_dictgen::catalog::build_catalog(
        &args.list,
        &args.books,
        args.dictionary.as_deref(),
        &args.language,
        &args.output,
        args.min_count,
    ) {
        Ok(summary) => {
            let size = std::fs::metadata(&args.output)
                .map(|m| m.len())
                .unwrap_or(0);
            println!(
                "{}: {} books, {} running words, {} KB",
                args.output.display(),
                summary.books,
                summary.running_words,
                size / 1024
            );
            for skipped in &summary.skipped {
                println!("skipped {skipped}");
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("catalog: {message}");
            ExitCode::FAILURE
        }
    }
}
