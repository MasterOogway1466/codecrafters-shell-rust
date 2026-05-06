use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use rustyline::completion::Completer;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{CompletionType, Config, Context, Editor, Helper, Result as RlResult};

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
        .build();
    let mut rl = Editor::with_config(config).unwrap();
    rl.set_helper(Some(ShellHelper));
    rl
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
    matches
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
                    let suffix = if is_dir { "/" } else { "" };
                    matches.push(format!("{}{}{}", dir, name, suffix));
                }
            }
        }
    }

    matches.sort();
    matches
}
