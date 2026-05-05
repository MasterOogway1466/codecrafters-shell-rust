mod completion;
mod exec;
mod parser;
mod pipeline;

use std::io::{self, Write};

use completion::read_line_with_tab;
use exec::eval_command;
use parser::{parse_input, parse_redirects};
use pipeline::run_pipeline;

pub const BUILTINS: &[&str] = &["exit", "echo", "type", "pwd", "cd"];

fn main() {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        let input = read_line_with_tab();

        let tokens = parse_input(&input);
        if tokens.is_empty() {
            continue;
        }
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
            eval_command(cmd, args, &redirect);
        }
    }
}
