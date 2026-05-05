use std::env;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;

use nix::libc;
use nix::sys::termios::{self, LocalFlags, SetArg};
use nix::sys::wait::waitpid;
use nix::unistd::{self, ForkResult};

const BUILTINS: &[&str] = &["exit", "echo", "type", "pwd", "cd"];

struct Redirect {
    stdout_file: Option<String>,
    stdout_append: bool,
    stderr_file: Option<String>,
    stderr_append: bool,
}

fn parse_redirects(tokens: Vec<String>) -> (Vec<String>, Redirect) {
    let mut args = Vec::new();
    let mut redirect = Redirect {
        stdout_file: None,
        stdout_append: false,
        stderr_file: None,
        stderr_append: false,
    };
    let mut iter = tokens.into_iter();

    while let Some(token) = iter.next() {
        if token == ">>" || token == "1>>" {
            if let Some(file) = iter.next() {
                redirect.stdout_file = Some(file);
                redirect.stdout_append = true;
            }
        } else if token == ">" || token == "1>" {
            if let Some(file) = iter.next() {
                redirect.stdout_file = Some(file);
                redirect.stdout_append = false;
            }
        } else if token == "2>>" {
            if let Some(file) = iter.next() {
                redirect.stderr_file = Some(file);
                redirect.stderr_append = true;
            }
        } else if token == "2>" {
            if let Some(file) = iter.next() {
                redirect.stderr_file = Some(file);
                redirect.stderr_append = false;
            }
        } else {
            args.push(token);
        }
    }

    (args, redirect)
}

fn read_line_with_tab() -> String {
    let stdin = io::stdin();
    let orig_termios = termios::tcgetattr(&stdin).unwrap();

    let mut raw = orig_termios.clone();
    raw.local_flags &= !(LocalFlags::ICANON | LocalFlags::ECHO);
    termios::tcsetattr(&stdin, SetArg::TCSANOW, &raw).unwrap();

    let mut input = String::new();
    let mut buf = [0u8; 1];
    let mut tab_count = 0u8;
    let mut last_tab_input = String::new();

    loop {
        io::stdin().read_exact(&mut buf).unwrap();
        match buf[0] {
            b'\n' => {
                print!("\n");
                io::stdout().flush().unwrap();
                break;
            }
            b'\t' => {
                if input != last_tab_input {
                    tab_count = 0;
                    last_tab_input = input.clone();
                }
                tab_count += 1;

                let matches = find_completions(&input);
                if matches.len() == 1 {
                    let suffix = matches[0][input.len()..].to_string();
                    input = matches[0].clone();
                    input.push(' ');
                    print!("{} ", suffix);
                    io::stdout().flush().unwrap();
                    tab_count = 0;
                } else if matches.len() > 1 {
                    let lcp = longest_common_prefix(&matches);
                    if lcp.len() > input.len() {
                        let suffix = lcp[input.len()..].to_string();
                        input = lcp;
                        print!("{}", suffix);
                        io::stdout().flush().unwrap();
                        last_tab_input = input.clone();
                        tab_count = 0;
                    } else if tab_count == 1 {
                        print!("\x07");
                        io::stdout().flush().unwrap();
                    } else {
                        print!("\n{}\n$ {}", matches.join("  "), input);
                        io::stdout().flush().unwrap();
                    }
                } else {
                    print!("\x07");
                    io::stdout().flush().unwrap();
                }
            }
            127 | 8 => {
                // Backspace
                if !input.is_empty() {
                    input.pop();
                    print!("\x08 \x08");
                    io::stdout().flush().unwrap();
                }
                tab_count = 0;
            }
            c => {
                input.push(c as char);
                print!("{}", c as char);
                io::stdout().flush().unwrap();
                tab_count = 0;
            }
        }
    }

    termios::tcsetattr(&stdin, SetArg::TCSANOW, &orig_termios).unwrap();
    input
}

fn find_completions(partial: &str) -> Vec<String> {
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

fn longest_common_prefix(strings: &[String]) -> String {
    if strings.is_empty() {
        return String::new();
    }
    let first = &strings[0];
    let mut len = first.len();
    for s in &strings[1..] {
        len = len.min(s.len());
        for (i, (a, b)) in first.chars().zip(s.chars()).enumerate() {
            if a != b {
                len = len.min(i);
                break;
            }
        }
    }
    first[..len].to_string()
}

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
                let file = open_redirect_file(file_path, redirect.stdout_append);
                unsafe { libc::dup2(file.as_raw_fd(), libc::STDOUT_FILENO); }
            }
            if let Some(ref file_path) = redirect.stderr_file {
                let file = open_redirect_file(file_path, redirect.stderr_append);
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
        open_redirect_file(file_path, redirect.stderr_append);
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
                write_error(&format!("cd: {}: No such file or directory", target), redirect);
            }
        }
        _ => run_external(command, args, redirect),
    }
}

fn open_redirect_file(path: &str, append: bool) -> File {
    if append {
        OpenOptions::new().create(true).append(true).open(path).unwrap()
    } else {
        File::create(path).unwrap()
    }
}

fn write_output(output: &str, redirect: &Redirect) {
    match redirect.stdout_file {
        Some(ref file_path) => {
            let mut file = open_redirect_file(file_path, redirect.stdout_append);
            writeln!(file, "{}", output).unwrap();
        }
        None => println!("{}", output),
    }
}

fn write_error(output: &str, redirect: &Redirect) {
    match redirect.stderr_file {
        Some(ref file_path) => {
            let mut file = open_redirect_file(file_path, redirect.stderr_append);
            writeln!(file, "{}", output).unwrap();
        }
        None => eprintln!("{}", output),
    }
}