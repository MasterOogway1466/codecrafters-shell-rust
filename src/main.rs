use std::env;
use std::ffi::CString;
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;

use nix::libc;
use nix::sys::wait::waitpid;
use nix::unistd::{self, ForkResult};

const BUILTINS: &[&str] = &["exit", "echo", "type", "pwd", "cd"];

struct Redirect {
    stdout_file: Option<String>,
    stderr_file: Option<String>,
}

fn parse_redirects(tokens: Vec<String>) -> (Vec<String>, Redirect) {
    let mut args = Vec::new();
    let mut redirect = Redirect { stdout_file: None, stderr_file: None };
    let mut iter = tokens.into_iter();

    while let Some(token) = iter.next() {
        if token == ">" || token == "1>" {
            if let Some(file) = iter.next() {
                redirect.stdout_file = Some(file);
            }
        } else if token == "2>" {
            if let Some(file) = iter.next() {
                redirect.stderr_file = Some(file);
            }
        } else {
            args.push(token);
        }
    }

    (args, redirect)
}

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

        let (tokens, redirect) = parse_redirects(tokens);
        let (cmd, args) = tokens.split_first().unwrap();
        eval_command(cmd, args, &redirect);
    }
}

fn parse_input(input: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if in_single_quote {
            if c == '\'' {
                in_single_quote = false;
            } else {
                current.push(c);
            }
        } else if in_double_quote {
            if c == '"' {
                in_double_quote = false;
            } else if c == '\\' {
                match chars.peek() {
                    Some(&next) if next == '"' || next == '\\' || next == '$' || next == '`' || next == '\n' => {
                        current.push(next);
                        chars.next();
                    }
                    _ => current.push(c),
                }
            } else {
                current.push(c);
            }
        } else {
            match c {
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                '\'' => in_single_quote = true,
                '"' => in_double_quote = true,
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

fn run_external(command: &str, args: &[String], redirect: &Redirect) {
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
            if let Some(ref file_path) = redirect.stdout_file {
                let file = File::create(file_path).unwrap();
                unsafe { libc::dup2(file.as_raw_fd(), libc::STDOUT_FILENO); }
            }
            if let Some(ref file_path) = redirect.stderr_file {
                let file = File::create(file_path).unwrap();
                unsafe { libc::dup2(file.as_raw_fd(), libc::STDERR_FILENO); }
            }
            let _ = unistd::execvp(&c_path, &c_args);
            std::process::exit(1);
        }
        Err(_) => panic!("fork failed"),
    }
}

fn eval_command(command: &str, args: &[String], redirect: &Redirect) {
    if let Some(ref file_path) = redirect.stderr_file {
        File::create(file_path).unwrap();
    }

    match command {
        "echo" => {
            let output = args.join(" ");
            write_output(&output, redirect);
        }
        "type" => {
            let target = &args[0];
            let output = if BUILTINS.contains(&target.as_str()) {
                format!("{} is a shell builtin", target)
            } else {
                match find_in_path(target) {
                    Some(path) => format!("{} is {}", target, path),
                    None => format!("{}: not found", target),
                }
            };
            write_output(&output, redirect);
        }
        "pwd" => match env::current_dir() {
            Ok(path) => write_output(&path.display().to_string(), redirect),
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
        _ => run_external(command, args, redirect),
    }
}

fn write_output(output: &str, redirect: &Redirect) {
    match redirect.stdout_file {
        Some(ref file_path) => {
            let mut file = File::create(file_path).unwrap();
            writeln!(file, "{}", output).unwrap();
        }
        None => println!("{}", output),
    }
}