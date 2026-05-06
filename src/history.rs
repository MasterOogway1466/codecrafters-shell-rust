use std::cell::Cell;
use std::fs::{self, OpenOptions};
use std::io::Write;

use rustyline::history::History;
use rustyline::Editor;

use crate::completion::ShellHelper;

thread_local! {
    static LAST_APPENDED: Cell<usize> = Cell::new(0);
}

pub fn print_history(rl: &Editor<ShellHelper, rustyline::history::DefaultHistory>, n: Option<usize>) {
    let hist = rl.history();
    let total = hist.len();
    let start = match n {
        Some(count) => total.saturating_sub(count),
        None => 0,
    };
    for (i, entry) in hist.iter().enumerate().skip(start) {
        println!("{:>5}  {}", i + 1, entry);
    }
}

pub fn load_history_file(rl: &mut Editor<ShellHelper, rustyline::history::DefaultHistory>, path: &str) {
    if let Ok(contents) = fs::read_to_string(path) {
        for line in contents.lines() {
            if !line.is_empty() {
                let _ = rl.add_history_entry(line);
            }
        }
    }
}

pub fn mark_appended(rl: &Editor<ShellHelper, rustyline::history::DefaultHistory>) {
    LAST_APPENDED.with(|c| c.set(rl.history().len()));
}

pub fn write_history_file(rl: &Editor<ShellHelper, rustyline::history::DefaultHistory>, path: &str) {
    let hist = rl.history();
    let mut content = String::new();
    for entry in hist.iter() {
        content.push_str(entry);
        content.push('\n');
    }
    let _ = fs::write(path, content);
}

pub fn append_history_file(rl: &Editor<ShellHelper, rustyline::history::DefaultHistory>, path: &str) {
    let hist = rl.history();
    let total = hist.len();
    let last = LAST_APPENDED.with(|c| c.get());

    if total > last {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        for entry in hist.iter().skip(last) {
            writeln!(file, "{}", entry).unwrap();
        }
        LAST_APPENDED.with(|c| c.set(total));
    }
}
