mod completion;
mod exec;
mod history;
mod parser;
mod pipeline;

use completion::build_editor;
use exec::eval_command;
use parser::{parse_input, parse_redirects};
use pipeline::run_pipeline;

pub const BUILTINS: &[&str] = &["exit", "echo", "type", "pwd", "cd", "history"];

fn main() {
    let mut rl = build_editor();

    loop {
        let readline = rl.readline("$ ");
        match readline {
            Ok(input) => {
                let input = input.trim_end().to_string();
                let tokens = parse_input(&input);
                if tokens.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(&input);

                if tokens[0] == "exit" {
                    break;
                }

                // Split tokens at pipe operators
                let commands: Vec<Vec<String>> = tokens
                    .split(|t| t == "|")
                    .map(|s| s.to_vec())
                    .collect();

                if commands.len() > 1 {
                    run_pipeline(&commands);
                } else {
                    let (tokens, redirect) = parse_redirects(commands.into_iter().next().unwrap());
                    let (cmd, args) = tokens.split_first().unwrap();
                    if cmd == "history" {
                        let n = args.first().and_then(|s| s.parse::<usize>().ok());
                        history::print_history(&rl, n);
                    } else {
                        eval_command(cmd, args, &redirect);
                    }
                }
            }
            Err(_) => break,
        }
    }
}
