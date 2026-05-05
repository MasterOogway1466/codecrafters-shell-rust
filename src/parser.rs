use std::fs::{File, OpenOptions};
use std::io::Write;

pub struct Redirect {
    pub stdout_file: Option<String>,
    pub stdout_append: bool,
    pub stderr_file: Option<String>,
    pub stderr_append: bool,
}

pub fn parse_input(input: &str) -> Vec<String> {
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

pub fn parse_redirects(tokens: Vec<String>) -> (Vec<String>, Redirect) {
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

pub fn open_redirect_file(path: &str, append: bool) -> File {
    if append {
        OpenOptions::new().create(true).append(true).open(path).unwrap()
    } else {
        File::create(path).unwrap()
    }
}

pub fn write_output(output: &str, redirect: &Redirect) {
    match redirect.stdout_file {
        Some(ref file_path) => {
            let mut file = open_redirect_file(file_path, redirect.stdout_append);
            writeln!(file, "{}", output).unwrap();
        }
        None => println!("{}", output),
    }
}

pub fn write_error(output: &str, redirect: &Redirect) {
    match redirect.stderr_file {
        Some(ref file_path) => {
            let mut file = open_redirect_file(file_path, redirect.stderr_append);
            writeln!(file, "{}", output).unwrap();
        }
        None => eprintln!("{}", output),
    }
}
