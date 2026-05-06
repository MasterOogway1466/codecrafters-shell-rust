use std::cell::RefCell;
use std::ffi::CString;

use nix::libc;
use nix::unistd::{self, ForkResult, Pid};

use crate::exec::find_in_path;

#[derive(Clone)]
struct Job {
    id: usize,
    pid: Pid,
    command: String,
}

thread_local! {
    static JOBS: RefCell<Vec<Job>> = RefCell::new(Vec::new());
    static NEXT_JOB_ID: RefCell<usize> = RefCell::new(1);
}

pub fn run_background(command: &str, args: &[String]) {
    let Some(path) = find_in_path(command) else {
        eprintln!("{}: command not found", command);
        return;
    };

    let c_path = CString::new(path).unwrap();
    let c_args: Vec<CString> = std::iter::once(command)
        .chain(args.iter().map(|s| s.as_str()))
        .map(|s| CString::new(s).unwrap())
        .collect();

    let full_command = std::iter::once(command.to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ");

    match unsafe { unistd::fork() } {
        Ok(ForkResult::Parent { child }) => {
            let job_id = NEXT_JOB_ID.with(|id| {
                let current = *id.borrow();
                *id.borrow_mut() = current + 1;
                current
            });
            JOBS.with(|jobs| {
                jobs.borrow_mut().push(Job {
                    id: job_id,
                    pid: child,
                    command: full_command,
                });
            });
            println!("[{}] {}", job_id, child);
        }
        Ok(ForkResult::Child) => {
            unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL); }
            let _ = unistd::execvp(&c_path, &c_args);
            std::process::exit(1);
        }
        Err(_) => panic!("fork failed"),
    }
}

pub fn print_jobs() {
    JOBS.with(|jobs| {
        for job in jobs.borrow().iter() {
            println!("[{}]  Running                    {} &", job.id, job.command);
        }
    });
}
