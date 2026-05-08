use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::ffi::CString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;

use nix::libc;
use nix::sys::wait::waitpid;
use nix::unistd::{self, ForkResult};

use crate::parser::{open_redirect_file, write_error, write_output, Redirect};
use crate::jobs;
use crate::BUILTINS;

thread_local! {
    static COMPLETIONS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

pub fn get_completer(command: &str) -> Option<String> {
    COMPLETIONS.with(|c| c.borrow().get(command).cloned())
}

pub fn find_in_path(name: &str) -> Option<String> {
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

pub fn eval_command(command: &str, args: &[String], redirect: &Redirect) {
    if let Some(ref file_path) = redirect.stderr_file {
        open_redirect_file(file_path, redirect.stderr_append);
    }

    match command {
        "echo" => write_output(&args.join(" "), redirect),
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
            Err(_) => eprintln!("Error getting current directory"),
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
        "history" => {} // handled in main.rs (needs rl access)
        "jobs" => jobs::print_jobs(),
        "complete" => handle_complete(args),
        _ => run_external(command, args, redirect),
    }
}

fn handle_complete(args: &[String]) {
    match args.first().map(|s| s.as_str()) {
        Some("-p") => {
            if let Some(cmd_name) = args.get(1) {
                match get_completer(cmd_name) {
                    Some(path) => println!("complete -C '{}' {}", path, cmd_name),
                    None => eprintln!("complete: {}: no completion specification", cmd_name),
                }
            }
        }
        Some("-C") => {
            if let (Some(path), Some(cmd_name)) = (args.get(1), args.get(2)) {
                COMPLETIONS.with(|c| c.borrow_mut().insert(cmd_name.clone(), path.clone()));
            }
        }
        Some("-r") => {
            if let Some(cmd_name) = args.get(1) {
                COMPLETIONS.with(|c| c.borrow_mut().remove(cmd_name));
            }
        }
        _ => {}
    }
}
