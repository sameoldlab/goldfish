/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use clap::Parser;
use ignore::WalkState;
use nucleo::{
    Nucleo,
    pattern::{CaseMatching, Normalization},
};
use std::{
    io::{self, BufRead, Write}, sync::Arc, thread, time::Instant
};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Search pattern used to search
    #[arg(short = 'q', long = "query")]
    pattern: Option<String>,

    /// Path to search. Defaults to current directory.
    path: Option<String>,

    /// Make searching case-insensitive default (smart-case)
    #[arg(short, long, default_value_t = false)]
    ignore_case: bool,

    /// Make searching case-insensitive
    #[arg(short, long, default_value_t = true)]
    smart_case: bool,

    /// disable all default ignored files (.gitignore, target, node_modules)
    #[arg(short = 'A', long, default_value_t = false)]
    no_ignore: bool,

    /// search hidden files
    #[arg(short = 'H', long, default_value_t = false)]
    hidden: bool,

    /// follow symbolic links
    #[arg(short = 'L', long = "follow", default_value_t = false)]
    follow_symlinks: bool,
}

fn main() -> Result<(), io::Error> {
    let cli = Cli::parse();
    let path = cli.path.unwrap_or(".".to_string());

    let mut m: Nucleo<String> = Nucleo::new(
        nucleo::Config::DEFAULT.match_paths(),
        Arc::new(|| {}),
        None,
        1,
    );
    let inj = Arc::new(m.injector());

    thread::spawn(move || {
        ignore::WalkBuilder::new(path)
            .require_git(false)
            .follow_links(cli.follow_symlinks)
            .standard_filters(!cli.no_ignore)
            .hidden(!cli.hidden)
            .threads(thread::available_parallelism().unwrap().get())
            .build_parallel()
            .run(|| {
                let inj = inj.clone();
                Box::new(move |entry| {
                    let entry = match entry {
                        Ok(e) => e.into_path(),
                        Err(_) => return WalkState::Continue,
                    };
                    // println!("{}", &entry.to_str().unwrap());
                    inj.push(entry.to_string_lossy().into(), |e, cols| {
                        cols[0] = e.to_owned().into()
                    });
                    WalkState::Continue
                })
            });
    });

    interactive(&mut m)?;
    Ok(())
}

fn interactive(m: &mut Nucleo<String>) -> Result<(), io::Error> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let reader = io::BufReader::new(stdin);
    let mut last_query = String::new();

    for line in reader.lines() {
        let msg = line?;
        if let Some(cmd) = msg.strip_prefix("c:") {
            match cmd {
                "Exit" => break,
                _ => (),
            }
        } else if let Some(query) = msg.strip_prefix("q:") {
            if query == last_query {
                continue;
            }

            m.pattern.reparse(
                0,
                query,
                CaseMatching::Smart,
                Normalization::Smart,
                query.starts_with(&last_query),
            );
            last_query = query.to_string();

            let loop_time = Instant::now();
            loop {
                let s = m.tick(10);

                if !s.running || loop_time.elapsed().as_millis() > 900 as u128 {
                    if s.changed {
                        let snapshot = m.snapshot();
                        let count = 10.min(snapshot.matched_item_count());
                        for result in snapshot.matched_items(..count) {
                            stdout.write(result.data.as_bytes())?;
                            stdout.write(b"\n")?;
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
