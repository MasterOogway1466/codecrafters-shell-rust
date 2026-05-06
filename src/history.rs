use rustyline::history::History;
use rustyline::Editor;

use crate::completion::ShellHelper;

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
