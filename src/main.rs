use std::io::{self, Write};

const VALID_COMMANDS: [&str; 3] = ["exit", "echo", "type"];

fn main() {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        let mut command = String::new();
        io::stdin().read_line(&mut command).unwrap();
        let command = command.trim();

        let (name, arg) = command
            .split_once(' ')
            .map(|(n, a)| (n, a.trim()))
            .unwrap_or((command, ""));

        match name {
            "exit" => break,
            "echo" => println!("{}", arg),
            "type" => match VALID_COMMANDS.contains(&arg) {
                true => println!("{} is a shell builtin", arg),
                false => println!("{}: not found", arg),
            },
            _ => println!("{}: command not found", name),
        }
    }
}
