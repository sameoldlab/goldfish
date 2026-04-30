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

use lexopt::Arg::{Long, Short, Value};
use nucleo::{
    Nucleo,
    pattern::{CaseMatching, Normalization},
};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
    sync::Arc,
    time::Instant,
};

use cache::open as cache_open;
use item::{SearchItem, SearchTarget};
use source::{WalkOptions, WalkSource};

fn cache_path() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("goldfish")
        .join("index.db")
}

struct Cli {

    /// Path to search (default: current directory)
    path: PathBuf,
    /// Case-insensitive matching
    ignore_case: bool,

    /// Disable default ignore rules (.gitignore, target, node_modules)
    no_ignore: bool,

    /// Include hidden files
    hidden: bool,

    /// Follow symbolic links
    follow_symlinks: bool,
}

fn parse_args() -> Result<Cli, lexopt::Error> {
    let mut path = None;
    let mut ignore_case = false;
    let mut no_ignore = false;
    let mut hidden = false;
    let mut follow_symlinks = false;

    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Short('i') | Long("ignore-case") => ignore_case = true,
            Short('A') | Long("no-ignore") => no_ignore = true,
            Short('H') | Long("hidden") => hidden = true,
            Short('L') | Long("follow") => follow_symlinks = true,
            Short('h') | Long("help") => {
                print!(concat!(
                    "Usage: fsearch [OPTIONS] [PATH]\n\n",
                    "Options:\n",
                    "  -i, --ignore-case  Case-insensitive matching\n",
                    "  -A, --no-ignore    Disable .gitignore and default filters\n",
                    "  -H, --hidden       Include hidden files\n",
                    "  -L, --follow       Follow symbolic links\n",
                    "  -h, --help         Print help\n",
                ));
                std::process::exit(0);
            }
            Value(v) if path.is_none() => path = Some(PathBuf::from(v)),
            arg => return Err(arg.unexpected()),
        }
    }

    Ok(Cli {
        path: path.unwrap_or_else(|| PathBuf::from(".")),
        ignore_case,
        no_ignore,
        hidden,
        follow_symlinks,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = parse_args()?;
    let root = cli.path;

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

    let db_path = cache_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
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

fn interactive(nucleo: &mut Nucleo<SearchItem>, case: CaseMatching) -> Result<(), io::Error> {
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
