use std::env;
use std::ffi::CString;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use nix::sys::wait::waitpid;
use nix::unistd::{self, ForkResult};

const BUILTINS: &[&str] = &["exit", "echo", "type", "pwd", "cd"];

fn main() {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        input.pop();

        let tokens = parse_input(&input);
        if tokens.is_empty() {
            continue;
        }
        if tokens[0] == "exit" {
            break;
        }

        let (cmd, args) = tokens.split_first().unwrap();
        eval_command(cmd, args);
    }
}

fn parse_input(input: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;

    for c in input.chars() {
        if in_single_quote {
            if c == '\'' {
                in_single_quote = false;
            } else {
                current.push(c);
            }
        } else {
            match c {
                '\'' => in_single_quote = true,
                ' ' | '\t' => {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                }
                _ => current.push(c),
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn find_in_path(name: &str) -> Option<String> {
    let path_env = env::var("PATH").ok()?;
    path_env.split(':').find_map(|dir| {
        let full_path = format!("{}/{}", dir, name);
        let metadata = fs::metadata(&full_path).ok()?;
        if metadata.permissions().mode() & 0o111 != 0 {
            Some(full_path)
        } else {
            None
        }
    })
}

fn resolve_home(path: &str) -> String {
    let home = env::var("HOME").unwrap_or_else(|_| "/".to_string());
    if path == "~" {
        home
    } else if path.starts_with("~/") {
        format!("{}/{}", home, &path[2..])
    } else {
        path.to_string()
    }
}

fn run_external(command: &str, args: &[String]) {
    let Some(path) = find_in_path(command) else {
        println!("{}: command not found", command);
        return;
    };

    let c_path = CString::new(path).unwrap();
    let c_args: Vec<CString> = std::iter::once(command)
        .chain(args.iter().map(|s| s.as_str()))
        .map(|s| CString::new(s).unwrap())
        .collect();

    match unsafe { unistd::fork() } {
        Ok(ForkResult::Parent { child }) => {
            let _ = waitpid(child, None);
        }
        Ok(ForkResult::Child) => {
            let _ = unistd::execvp(&c_path, &c_args);
            std::process::exit(1);
        }
        Err(_) => panic!("fork failed"),
    }
}

fn eval_command(command: &str, args: &[String]) {
    match command {
        "echo" => println!("{}", args.join(" ")),
        "type" => {
            let target = &args[0];
            if BUILTINS.contains(&target.as_str()) {
                println!("{} is a shell builtin", target);
            } else {
                match find_in_path(target) {
                    Some(path) => println!("{} is {}", target, path),
                    None => println!("{}: not found", target),
                }
            }
        }
        "pwd" => match env::current_dir() {
            Ok(path) => println!("{}", path.display()),
            Err(_) => println!("Error getting current directory"),
        },
        "cd" => {
            let target = if args.is_empty() {
                resolve_home("~")
            } else {
                resolve_home(&args[0])
            };
            if env::set_current_dir(Path::new(&target)).is_err() {
                println!("cd: {}: No such file or directory", target);
            }
        }
        _ => run_external(command, args),
    }
}