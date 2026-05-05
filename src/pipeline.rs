use std::ffi::CString;
use std::os::unix::io::AsRawFd;

use nix::libc;
use nix::sys::wait::waitpid;
use nix::unistd::{self, close, pipe, ForkResult};

use crate::exec::find_in_path;
use crate::parser::{open_redirect_file, parse_redirects};

pub fn run_pipeline(commands: &[Vec<String>]) {
    let n = commands.len();
    let mut prev_read_fd: Option<i32> = None;
    let mut child_pids = Vec::new();

    for (i, cmd_tokens) in commands.iter().enumerate() {
        let (tokens, redirect) = parse_redirects(cmd_tokens.clone());
        if tokens.is_empty() {
            continue;
        }
        let command = &tokens[0];
        let args = &tokens[1..];

        let is_last = i == n - 1;

        // Create pipe for all but the last command
        let pipe_fds = if !is_last {
            let (read_fd, write_fd) = pipe().unwrap();
            Some((read_fd, write_fd))
        } else {
            None
        };

        let Some(path) = find_in_path(command) else {
            eprintln!("{}: command not found", command);
            if let Some(fd) = prev_read_fd {
                close(fd).ok();
            }
            if let Some((r, w)) = pipe_fds {
                close(r).ok();
                close(w).ok();
            }
            return;
        };

        let c_path = CString::new(path).unwrap();
        let c_args: Vec<CString> = std::iter::once(command.as_str())
            .chain(args.iter().map(|s| s.as_str()))
            .map(|s| CString::new(s).unwrap())
            .collect();

        match unsafe { unistd::fork() } {
            Ok(ForkResult::Parent { child }) => {
                if let Some(fd) = prev_read_fd {
                    close(fd).ok();
                }
                if let Some((ref _r, ref w)) = pipe_fds {
                    close(w.as_raw_fd()).ok();
                }
                prev_read_fd = pipe_fds.map(|(r, _)| r.as_raw_fd());
                child_pids.push(child);
            }
            Ok(ForkResult::Child) => {
                if let Some(fd) = prev_read_fd {
                    unsafe { libc::dup2(fd, libc::STDIN_FILENO); }
                    close(fd).ok();
                }
                if let Some((r, w)) = pipe_fds {
                    close(r).ok();
                    unsafe { libc::dup2(w.as_raw_fd(), libc::STDOUT_FILENO); }
                    close(w).ok();
                }
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

    for pid in child_pids {
        let _ = waitpid(pid, None);
    }
}
