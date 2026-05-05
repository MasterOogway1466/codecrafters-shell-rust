mod completion;
mod exec;
mod parser;

use std::io::{self, Write};

use completion::read_line_with_tab;
use exec::eval_command;
use parser::{parse_input, parse_redirects};

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

        let (tokens, redirect) = parse_redirects(tokens);
        let (cmd, args) = tokens.split_first().unwrap();
        eval_command(cmd, args, &redirect);
    }
}
