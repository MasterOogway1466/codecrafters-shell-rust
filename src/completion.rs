use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;

use nix::sys::termios::{self, LocalFlags, SetArg};

use crate::BUILTINS;

pub fn read_line_with_tab() -> String {
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
