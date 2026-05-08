mod completion;
mod declare;
mod exec;
mod history;
mod jobs;
mod parser;
mod pipeline;

use std::env;

use completion::build_editor;
use exec::eval_command;
use parser::{expand_variables, parse_input, parse_redirects};
use pipeline::run_pipeline;
use rustyline::Editor;

use crate::completion::ShellHelper;

pub const BUILTINS: &[&str] = &["exit", "echo", "type", "pwd", "cd", "history", "jobs", "complete", "declare"];

pub type Rl = Editor<ShellHelper, rustyline::history::DefaultHistory>;

fn main() {
    let mut rl = build_editor();

    if let Ok(histfile) = env::var("HISTFILE") {
        history::load_history_file(&mut rl, &histfile);
        history::mark_appended(&rl);
    }

    loop {
        jobs::reap_jobs();

        let Ok(input) = rl.readline("$ ") else { break };
        let input = input.trim_end().to_string();
        let tokens = expand_variables(parse_input(&input));
        if tokens.is_empty() {
            continue;
        }

        let _ = rl.add_history_entry(&input);

        if tokens[0] == "exit" {
            break;
        }

        // Background execution
        if tokens.last().is_some_and(|t| t == "&") {
            let tokens = &tokens[..tokens.len() - 1];
            jobs::run_background(&tokens[0], &tokens[1..].to_vec());
            continue;
        }

        // Pipeline
        let commands: Vec<Vec<String>> = tokens.split(|t| t == "|").map(|s| s.to_vec()).collect();
        if commands.len() > 1 {
            run_pipeline(&commands);
            continue;
        }

        // Single command
        let (tokens, redirect) = parse_redirects(commands.into_iter().next().unwrap());
        let (cmd, args) = tokens.split_first().unwrap();

        match cmd.as_str() {
            "history" => handle_history(&mut rl, args),
            _ => eval_command(cmd, args, &redirect),
        }
    }

    if let Ok(histfile) = env::var("HISTFILE") {
        history::write_history_file(&rl, &histfile);
    }
}

fn handle_history(rl: &mut Rl, args: &[String]) {
    match args.first().map(|s| s.as_str()) {
        Some("-r") => {
            if let Some(path) = args.get(1) {
                history::load_history_file(rl, path);
            }
        }
        Some("-w") => {
            if let Some(path) = args.get(1) {
                history::write_history_file(rl, path);
            }
        }
        Some("-a") => {
            if let Some(path) = args.get(1) {
                history::append_history_file(rl, path);
            }
        }
        _ => {
            let n = args.first().and_then(|s| s.parse::<usize>().ok());
            history::print_history(rl, n);
        }
    }
}
