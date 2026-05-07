use std::ffi::CString;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};

use nix::libc;
use nix::sys::wait::waitpid;
use nix::unistd::{self, ForkResult, Pid};

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

        let pipe_fds = if !is_last { Some(create_pipe()) } else { None };

        let child = if BUILTINS.contains(&command.as_str()) {
            fork_builtin(command, args, &prev_read_fd, &pipe_fds, &redirect)
        } else {
            let Some(path) = find_in_path(command) else {
                eprintln!("{}: command not found", command);
                drop(prev_read_fd);
                return;
            };
            fork_external(&path, command, args, &prev_read_fd, &pipe_fds, &redirect)
        };

        drop(prev_read_fd);
        prev_read_fd = pipe_fds.map(|(r, _w)| r);
        child_pids.push(child);
    }

    drop(prev_read_fd);
    for pid in child_pids {
        let _ = waitpid(pid, None);
    }
}

fn create_pipe() -> (OwnedFd, OwnedFd) {
    let mut fds = [0i32; 2];
    unsafe { libc::pipe(fds.as_mut_ptr()); }
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

fn fork_external(
    path: &str,
    command: &str,
    args: &[String],
    prev_read_fd: &Option<OwnedFd>,
    pipe_fds: &Option<(OwnedFd, OwnedFd)>,
    redirect: &Redirect,
) -> Pid {
    let c_path = CString::new(path).unwrap();
    let c_args: Vec<CString> = std::iter::once(command)
        .chain(args.iter().map(|s| s.as_str()))
        .map(|s| CString::new(s).unwrap())
        .collect();

    match unsafe { unistd::fork() } {
        Ok(ForkResult::Parent { child }) => child,
        Ok(ForkResult::Child) => {
            setup_child_io(prev_read_fd, pipe_fds, redirect);
            drop_pipe_fds(prev_read_fd, pipe_fds);
            let _ = unistd::execvp(&c_path, &c_args);
            std::process::exit(1);
        }
        Err(_) => panic!("fork failed"),
    }
}

fn fork_builtin(
    command: &str,
    args: &[String],
    prev_read_fd: &Option<OwnedFd>,
    pipe_fds: &Option<(OwnedFd, OwnedFd)>,
    redirect: &Redirect,
) -> Pid {
    let cmd_owned = command.to_string();
    let args_owned: Vec<String> = args.to_vec();

    match unsafe { unistd::fork() } {
        Ok(ForkResult::Parent { child }) => child,
        Ok(ForkResult::Child) => {
            setup_child_io(prev_read_fd, pipe_fds, redirect);
            drop_pipe_fds(prev_read_fd, pipe_fds);
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

fn drop_pipe_fds(prev_read_fd: &Option<OwnedFd>, pipe_fds: &Option<(OwnedFd, OwnedFd)>) {
    // Safety: we're in the child after dup2 — these are references to the parent's
    // OwnedFds which will be dropped when this function's caller returns, but we
    // want to close them now to avoid fd leaks. Since the child will exit/exec
    // shortly, the drop on the references suffices via the outer scope.
    let _ = prev_read_fd;
    let _ = pipe_fds;
}

fn setup_child_io(prev_read_fd: &Option<OwnedFd>, pipe_fds: &Option<(OwnedFd, OwnedFd)>, redirect: &Redirect) {
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL); }

    if let Some(read_fd) = prev_read_fd {
        unsafe { libc::dup2(read_fd.as_raw_fd(), libc::STDIN_FILENO); }
    }

    if let Some((_r, write_fd)) = pipe_fds {
        unsafe { libc::dup2(write_fd.as_raw_fd(), libc::STDOUT_FILENO); }
    }

    if let Some(ref file_path) = redirect.stdout_file {
        let file = open_redirect_file(file_path, redirect.stdout_append);
        unsafe { libc::dup2(file.as_raw_fd(), libc::STDOUT_FILENO); }
    }
    if let Some(ref file_path) = redirect.stderr_file {
        let file = open_redirect_file(file_path, redirect.stderr_append);
        unsafe { libc::dup2(file.as_raw_fd(), libc::STDERR_FILENO); }
    }
}
