use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use rustyline::completion::Completer;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{CompletionType, Config, Context, Editor, Helper, Result as RlResult};

use crate::exec::get_completer;
use crate::BUILTINS;

#[derive(Clone)]
pub struct ShellHelper;

impl Helper for ShellHelper {}
impl Highlighter for ShellHelper {}
impl Hinter for ShellHelper {
    type Hint = String;
}
impl Validator for ShellHelper {}

impl Completer for ShellHelper {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> RlResult<(usize, Vec<String>)> {
        let input = &line[..pos];
        if input.contains(' ') {
            // Check if there's a registered completer for this command
            let words: Vec<&str> = input.split_whitespace().collect();
            let cmd = words.first().copied().unwrap_or("");
            if let Some(script) = get_completer(cmd) {
                let current_word = input.rsplit(' ').next().unwrap_or("");
                let prev_word = if words.len() >= 2 && !current_word.is_empty() {
                    words[words.len() - 2]
                } else if current_word.is_empty() && !words.is_empty() {
                    words[words.len() - 1]
                } else {
                    ""
                };
                // If line ends with space, current word is empty and prev is last word
                let (cur, prev) = if input.ends_with(' ') {
                    ("", words.last().copied().unwrap_or(""))
                } else {
                    (current_word, if words.len() >= 2 { words[words.len() - 2] } else { "" })
                };
                let completions = run_completer_script(&script, cmd, cur, prev);
                if !completions.is_empty() {
                    let last_space = input.rfind(' ').unwrap_or(0);
                    let start = last_space + 1;
                    return Ok((start, completions));
                }
            }
            let last_space = input.rfind(' ').unwrap_or(0);
            let start = last_space + 1;
            let completions = find_file_completions(input);
            Ok((start, completions))
        } else {
            let completions = find_command_completions(input);
            Ok((0, completions))
        }
    }
}

pub fn build_editor() -> Editor<ShellHelper, rustyline::history::DefaultHistory> {
    let config = Config::builder()
        .completion_type(CompletionType::List)
        .bell_style(rustyline::config::BellStyle::Audible)
        .max_history_size(10000)
        .unwrap()
        .history_ignore_dups(false)
        .unwrap()
        .build();
    let mut rl = Editor::with_config(config).unwrap();
    rl.set_helper(Some(ShellHelper));
    rl
}

fn run_completer_script(script: &str, cmd: &str, current_word: &str, prev_word: &str) -> Vec<String> {
    let output = Command::new(script)
        .arg(cmd)
        .arg(current_word)
        .arg(prev_word)
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| format!("{} ", l))
                .collect()
        }
        _ => Vec::new(),
    }
}

fn find_command_completions(partial: &str) -> Vec<String> {
    if partial.is_empty() {
        return Vec::new();
    }

    let mut matches: Vec<String> = BUILTINS
        .iter()
        .filter(|b| b.starts_with(partial) && **b != partial)
        .map(|b| b.to_string())
        .collect();

    if let Ok(path_env) = env::var("PATH") {
        for dir in path_env.split(':') {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.starts_with(partial) && name != partial && !matches.contains(&name.to_string()) {
                            if let Ok(meta) = entry.metadata() {
                                if meta.permissions().mode() & 0o111 != 0 {
                                    matches.push(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    matches.sort();
    matches.iter().map(|m| format!("{} ", m)).collect()
}

fn find_file_completions(input: &str) -> Vec<String> {
    let last_space = input.rfind(' ').unwrap_or(0);
    let partial_path = &input[last_space + 1..];

    let (dir, file_prefix) = if let Some(slash_pos) = partial_path.rfind('/') {
        (&partial_path[..slash_pos + 1], &partial_path[slash_pos + 1..])
    } else {
        ("", partial_path)
    };

    let search_dir = if dir.is_empty() { ".".to_string() } else { dir.to_string() };

    let mut matches: Vec<String> = Vec::new();

    if let Ok(entries) = fs::read_dir(&search_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                if name.starts_with(file_prefix) && (file_prefix.is_empty() || name != file_prefix || is_dir) {
                    let suffix = if is_dir { "/" } else { " " };
                    matches.push(format!("{}{}{}", dir, name, suffix));
                }
            }
        }
    }

    matches.sort();
    matches
}
