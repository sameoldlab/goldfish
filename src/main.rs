/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

mod cache;
mod error;
mod item;
mod pipeline;
mod source;

use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
    sync::Arc,
    time::Instant,
};

use clap::Parser;
use nucleo::{
    Nucleo,
    pattern::{CaseMatching, Normalization},
};

use cache::open as cache_open;
use item::{SearchItem, SearchTarget};
use source::{WalkOptions, WalkSource};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Search pattern
    #[arg(short = 'q', long = "query")]
    pattern: Option<String>,

    /// Path to search (default: current directory)
    path: Option<String>,

    /// Case-insensitive matching
    #[arg(short, long, default_value_t = false)]
    ignore_case: bool,

    /// Disable default ignore rules (.gitignore, target, node_modules)
    #[arg(short = 'A', long, default_value_t = false)]
    no_ignore: bool,

    /// Include hidden files
    #[arg(short = 'H', long, default_value_t = false)]
    hidden: bool,

    /// Follow symbolic links
    #[arg(short = 'L', long = "follow", default_value_t = false)]
    follow_symlinks: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let root = PathBuf::from(cli.path.unwrap_or_else(|| ".".to_string()));

    let column_count = 2u32;
    let case = if cli.ignore_case {
        CaseMatching::Ignore
    } else {
        CaseMatching::Smart
    };

    let mut nucleo: Nucleo<SearchItem> = Nucleo::new(
        nucleo::Config::DEFAULT.match_paths(),
        Arc::new(|| {}),
        None,
        column_count,
    );

    let injector = Arc::new(nucleo.injector());

    let db_path = root.join(".fsearch.db");
    let (writer, reader_factory) = cache_open(&db_path)?;

    let source = WalkSource {
        root: root.clone(),
        options: WalkOptions {
            standard_filters: !cli.no_ignore,
            hidden: cli.hidden,
            follow_symlinks: cli.follow_symlinks,
        },
    };

    std::thread::spawn(move || {
        if let Err(e) = pipeline::run(source, Arc::new(reader_factory), writer, injector) {
            eprintln!("pipeline error: {e}");
        }
    });

    interactive(&mut nucleo, case)?;
    Ok(())
}

fn interactive(
    nucleo: &mut Nucleo<SearchItem>,
    case: CaseMatching,
) -> Result<(), io::Error> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut last_query = String::new();

    for line in io::BufReader::new(stdin).lines() {
        let msg = line?;

        if let Some(cmd) = msg.strip_prefix("c:") {
            match cmd {
                "Exit" => break,
                _ => {}
            }
            continue;
        }

        if let Some(query) = msg.strip_prefix("q:") {
            if query == last_query {
                continue;
            }

            for col in 0..2 {
                nucleo.pattern.reparse(
                    col,
                    query,
                    case,
                    Normalization::Smart,
                    query.starts_with(&last_query),
                );
            }
            last_query = query.to_string();

            let deadline = Instant::now();
            loop {
                let status = nucleo.tick(10);
                if !status.running || deadline.elapsed().as_millis() > 900 {
                    if status.changed {
                        let snapshot = nucleo.snapshot();
                        let count = 10.min(snapshot.matched_item_count());
                        for result in snapshot.matched_items(..count) {
                            stdout.write_all(result.data.display().as_bytes())?;
                            stdout.write_all(b"\n")?;
                        }
                        stdout.flush()?;
                    }
                    break;
                }
            }
        }
    }

    Ok(())
}
