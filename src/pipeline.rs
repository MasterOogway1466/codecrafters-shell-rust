use std::ffi::CString;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};

use nix::libc;
use nix::sys::wait::waitpid;
use nix::unistd::{self, ForkResult};

use crate::exec::{eval_command, find_in_path};
use crate::parser::{open_redirect_file, parse_redirects, Redirect};
use crate::BUILTINS;

pub fn run_pipeline(commands: &[Vec<String>]) {
    let n = commands.len();
    let mut prev_read_fd: Option<OwnedFd> = None;
    let mut child_pids = Vec::new();

    for (i, cmd_tokens) in commands.iter().enumerate() {
        let (tokens, redirect) = parse_redirects(cmd_tokens.clone());
        if tokens.is_empty() {
            continue;
        }
        let command = &tokens[0];
        let args = &tokens[1..];

        let is_last = i == n - 1;
        let is_builtin = BUILTINS.contains(&command.as_str());

        // Create pipe for all but the last command
        let pipe_fds: Option<(OwnedFd, OwnedFd)> = if !is_last {
            let mut fds = [0i32; 2];
            unsafe { libc::pipe(fds.as_mut_ptr()); }
            unsafe { Some((OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1]))) }
        } else {
            None
        };

        if !is_builtin {
            let Some(path) = find_in_path(command) else {
                eprintln!("{}: command not found", command);
                drop(prev_read_fd);
                drop(pipe_fds);
                return;
            };

            let c_path = CString::new(path).unwrap();
            let c_args: Vec<CString> = std::iter::once(command.as_str())
                .chain(args.iter().map(|s| s.as_str()))
                .map(|s| CString::new(s).unwrap())
                .collect();

            match unsafe { unistd::fork() } {
                Ok(ForkResult::Parent { child }) => {
                    drop(prev_read_fd);
                    if let Some((r, _w)) = pipe_fds {
                        prev_read_fd = Some(r);
                    } else {
                        prev_read_fd = None;
                    }
                    child_pids.push(child);
                }
                Ok(ForkResult::Child) => {
                    setup_child_io(&prev_read_fd, &pipe_fds, &redirect);
                    drop(prev_read_fd);
                    drop(pipe_fds);
                    let _ = unistd::execvp(&c_path, &c_args);
                    std::process::exit(1);
                }
                Err(_) => panic!("fork failed"),
            }
        } else {
            // Fork for builtin so we can redirect its I/O in the pipeline
            let args_owned: Vec<String> = args.to_vec();
            let cmd_owned = command.clone();

            match unsafe { unistd::fork() } {
                Ok(ForkResult::Parent { child }) => {
                    drop(prev_read_fd);
                    if let Some((r, _w)) = pipe_fds {
                        prev_read_fd = Some(r);
                    } else {
                        prev_read_fd = None;
                    }
                    child_pids.push(child);
                }
                Ok(ForkResult::Child) => {
                    setup_child_io(&prev_read_fd, &pipe_fds, &redirect);
                    drop(prev_read_fd);
                    drop(pipe_fds);
                    // Run builtin with a no-redirect since I/O is already wired via dup2
                    let no_redirect = Redirect {
                        stdout_file: None,
                        stdout_append: false,
                        stderr_file: None,
                        stderr_append: false,
                    };
                    eval_command(&cmd_owned, &args_owned, &no_redirect);
                    std::process::exit(0);
                }
                Err(_) => panic!("fork failed"),
            }
        }
    }

    drop(prev_read_fd);

    for pid in child_pids {
        let _ = waitpid(pid, None);
    }
}

fn setup_child_io(prev_read_fd: &Option<OwnedFd>, pipe_fds: &Option<(OwnedFd, OwnedFd)>, redirect: &Redirect) {
    // Redirect stdin from previous pipe's read end
    if let Some(read_fd) = prev_read_fd {
        unsafe { libc::dup2(read_fd.as_raw_fd(), libc::STDIN_FILENO); }
    }

    // Redirect stdout to current pipe's write end
    if let Some((_r, write_fd)) = pipe_fds {
        unsafe { libc::dup2(write_fd.as_raw_fd(), libc::STDOUT_FILENO); }
    }

    // Apply file redirects
    if let Some(ref file_path) = redirect.stdout_file {
        let file = open_redirect_file(file_path, redirect.stdout_append);
        unsafe { libc::dup2(file.as_raw_fd(), libc::STDOUT_FILENO); }
    }
    if let Some(ref file_path) = redirect.stderr_file {
        let file = open_redirect_file(file_path, redirect.stderr_append);
        unsafe { libc::dup2(file.as_raw_fd(), libc::STDERR_FILENO); }
    }
}
