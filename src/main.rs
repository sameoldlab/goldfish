use clap::Parser;
use ignore::WalkState;
use nucleo::{
    Nucleo,
    pattern::{CaseMatching, Normalization},
};
use std::{
    io::{self, BufRead, Write},
    sync::Arc,
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

    let mut m = Matcher::new(nucleo::Config::DEFAULT.match_paths());
    let inj = Arc::new(m.injector());

    std::thread::spawn(move || {
        ignore::WalkBuilder::new(path)
            .require_git(false)
            .follow_links(cli.follow_symlinks)
            .standard_filters(!cli.no_ignore)
            .hidden(!cli.hidden)
            .threads(std::thread::available_parallelism().unwrap().get())
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

fn interactive(m: &mut Matcher) -> Result<(), io::Error> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let reader = io::BufReader::new(stdin);

    for line in reader.lines() {
        let query = line?;
        if query == "Exit" { break; }

        m.find(&query);

        // enter a loop checking for updates every 10 milliseconds 
        // updates are only sent if there is a change.
        // change and running flip at the same time to give a single result
        loop {
            let [changed, running] = m.tick();
            if !changed { continue }

            stdout.write(b"->")?;
            for result in m.results(10) {
                stdout.write(result.as_bytes())?;
            }
            stdout.write(b"\n")?;
            stdout.flush()?;

            if !running {break;}
        }
    }
    Ok(())
}

struct Matcher {
    inner: Nucleo<String>,
    pub last_pattern: String,
}

impl Matcher {
    pub fn new(config: nucleo::Config) -> Self {
        let cols = 1;
        Self {
            inner: Nucleo::new(config, Arc::new(|| {}), None, cols),
            last_pattern: String::new(),
        }
    }
    fn tick(&mut self) -> [bool; 2] {
        let status = self.inner.tick(10);
        [status.changed, status.running]
    }

    pub fn injector(&self) -> nucleo::Injector<String> {
        self.inner.injector()
    }

    pub fn find(&mut self, pattern: &str) {
        if pattern == self.last_pattern {
            return;
        }

        self.inner.pattern.reparse(
            0,
            pattern,
            CaseMatching::Smart,
            Normalization::Smart,
            pattern.starts_with(&self.last_pattern),
        );
        self.last_pattern = pattern.to_string();
    }

    fn results(&mut self, count: u32) -> Vec<&String> {
        let snapshot = self.inner.snapshot();
        // dbg!(&snapshot.matched_item_count());

        let count = if count > snapshot.matched_item_count() {
            snapshot.matched_item_count()
        } else {
            count
        };

        let mut results = Vec::with_capacity(count as usize);

        for entry in snapshot.matched_items(..count) {
            // snapshot.pattern().column_pattern(0).indices(entry.matcher_columns[0].slice(..), matcher, Vec::new())
            results.push(entry.data);
        }

        results
    }
}
