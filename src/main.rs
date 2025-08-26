use clap::Parser;
use nucleo::{
    Nucleo,
    pattern::{CaseMatching, Normalization},
};
use std::{path::PathBuf, sync::Arc};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Search pattern used to search
    pattern: String,

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
    show_all: bool,

    /// search hidden files
    #[arg(short = 'H', long, default_value_t = false)]
    hidden: bool,

    /// follow symbolic links
    #[arg(short = 'L', long = "follow", default_value_t = false)]
    follow_symlinks: bool,
}

fn main() {
    let cli = Cli::parse();
    let path = &cli.path.unwrap_or(".".to_string());

    let paths = traverse(
        &path,
        TraverseOpts {
            follow_symlinks: cli.follow_symlinks,
            require_git: false,
            hidden: cli.hidden,
            show_all: cli.show_all,
        },
    );

    let mut m = Matcher::new(nucleo::Config::DEFAULT.match_paths());
    m.inject( paths.map(|p| {p.to_string_lossy().into()}) );

    picker(&mut m, &cli.pattern );

    dbg!(&cli.pattern);
    dbg!(&path);
}

struct TraverseOpts {
    follow_symlinks: bool,
    require_git: bool,
    hidden: bool,
    show_all: bool,
}

fn traverse(dir: &str, opts: TraverseOpts) -> impl Iterator<Item = PathBuf> {
    ignore::WalkBuilder::new(dir)
        .require_git(opts.require_git)
        .follow_links(opts.follow_symlinks)
        .standard_filters(opts.show_all)
        .hidden(opts.hidden)
        .build()
        .into_iter()
        .filter_map(|e| e.ok())
        .map(ignore::DirEntry::into_path)
}

fn picker(matcher: &mut Matcher, pattern: &str) {
    matcher.find(pattern);
    matcher.tick();
    let res = matcher.results(100);
    dbg!(res);
}

struct Matcher {
    inner: Nucleo<String>,
    pub running: bool,
    pub last_pattern: String,
}

impl Matcher {
    pub fn new(config: nucleo::Config) -> Self {
        let cols = 1;
        Self {
            inner: Nucleo::new(config, Arc::new(|| {}), None, cols),
            running: false,
            last_pattern: String::new(),
        }
    }
    fn tick(&mut self) {
        let status = self.inner.tick(10);
        self.running = status.running;
    }

    pub fn inject(&self, entries: impl Iterator<Item = String>) {
        let injector = self.inner.injector();
        for entry in entries {
            injector.push(entry.into(), |s, cols| cols[0] = s.to_owned().into());
        }
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
        dbg!(&snapshot.matched_item_count());

        let count = if count > snapshot.matched_item_count() {
            snapshot.matched_item_count()
        } else {
            count
        };

        let mut results = Vec::with_capacity(count as usize);

        for entry in snapshot.matched_items(..count) {
            results.push(entry.data);
        }

        results
    }
}

