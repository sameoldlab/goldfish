use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute, style,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use nucleo::{
    Nucleo,
    pattern::{CaseMatching, Normalization},
};
use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
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
    let path = cli.path.as_deref().unwrap_or(".");

    let paths = traverse(
        &path,
        TraverseOpts {
            follow_symlinks: cli.follow_symlinks,
            require_git: false,
            filter_hidden: !cli.hidden,
            filter_all: !cli.no_ignore,
        },
    );

    if let Some(pattern) = cli.pattern {
        single_shot(paths, &pattern);
    } else if io::stdin().is_terminal() {
        interactive(paths)?;
    } else {
        pipe_mode(paths)?;
    }
    Ok(())
}

fn pipe_mode(
    paths: impl Iterator<Item = PathBuf>,
) -> Result<(), io::Error> {
    use std::io::BufRead;

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    let mut m = Matcher::new(nucleo::Config::DEFAULT.match_paths());
    m.inject(paths.map(|p| p.to_string_lossy().into()));

    // Read queries from stdin and respond
    for line in stdin.lock().lines() {
        let query = line?;
        m.find(&query);
        m.tick();

        let results = m.results(10);
        for entry in results {
            writeln!(stdout, "{entry}").unwrap();
        }
        stdout.flush()?;
    }

    Ok(())
}

fn interactive(paths: impl Iterator<Item = PathBuf>) -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let mut query = String::new();
    let mut selected = 0;
    let mut results = Vec::new();

    let mut m = Matcher::new(nucleo::Config::DEFAULT.match_paths());
    m.inject(paths.map(|p| p.to_string_lossy().into()));

    loop {
        m.find(&query);
        m.tick();

        results = m.results(50);

        render_ui(&mut stdout, &query, results.as_slice(), selected)?;
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char(c) => {
                    query.push(c);
                    selected = 0;
                }
                KeyCode::Backspace => {
                    query.pop();
                    selected = 0;
                }
                KeyCode::Up => {
                    if selected > 0 {
                        selected -= 1;
                    } else {
                        selected = results.len().saturating_sub(1);
                    }
                }
                KeyCode::Down => {
                    if selected < results.len().saturating_sub(1) {
                        selected += 1;
                    } else {
                        selected = 0;
                    }
                }
                KeyCode::Enter => {
                    disable_raw_mode()?;
                    execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
                    if let Some(item) = results.get(selected) {
                        println!("{}", item);
                        return Ok(());
                    }
                }
                KeyCode::Esc => break,
                _ => {}
            }
        }
    }
    disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;

    Ok(())
}

fn render_ui(
    stdout: &mut io::Stdout,
    query: &str,
    results: &[&String],
    // results: &[nucleo::Item<String>],
    selected: usize,
) -> Result<(), std::io::Error> {
    use crossterm::{cursor, terminal};

    execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0),
        cursor::Show,
        style::Print(query),
        cursor::MoveTo(0, 1),
        style::Print("-".repeat(50)),
        cursor::MoveTo(0, 1)
    )?;

    for (i, item) in results.iter().enumerate() {
        execute!(
            stdout,
            if i == selected {
                style::SetBackgroundColor(style::Color::DarkGreen)
            } else {
                style::SetBackgroundColor(style::Color::Reset)
            },
            style::Print(item),
            cursor::MoveTo(0, 2 + i as u16)
        )?;
    }

    execute!(stdout, cursor::MoveTo(query.len() as u16, 0))?;

    stdout.flush()?;
    Ok(())
}

fn single_shot(paths: impl Iterator<Item = PathBuf>, pattern: &str) {
    let mut m = Matcher::new(nucleo::Config::DEFAULT.match_paths());
    m.inject(paths.map(|p| p.to_string_lossy().into()));
    m.find(pattern);
    m.tick();

    let results = m.results(100);
    let mut stdout = io::stdout().lock();
    for entry in results {
        writeln!(stdout, "{entry}").unwrap();
    }
}

struct TraverseOpts {
    follow_symlinks: bool,
    require_git: bool,
    filter_hidden: bool,
    filter_all: bool,
}

fn traverse(dir: &str, opts: TraverseOpts) -> impl Iterator<Item = PathBuf> {
    ignore::WalkBuilder::new(dir)
        .require_git(opts.require_git)
        .follow_links(opts.follow_symlinks)
        .standard_filters(opts.filter_all)
        .hidden(opts.filter_hidden)
        .build()
        .into_iter()
        .filter_map(|e| e.ok())
        .map(ignore::DirEntry::into_path)
}

fn picker(matcher: &mut Matcher, pattern: &str) {
    matcher.find(pattern);
    matcher.tick();
    let res = matcher.results(100);
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
            injector.push(entry.into(), |e, cols| cols[0] = e.to_owned().into());
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
