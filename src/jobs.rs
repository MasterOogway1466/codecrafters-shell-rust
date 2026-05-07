use std::cell::RefCell;
use std::ffi::CString;

use nix::libc;
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::{self, ForkResult, Pid};

use crate::exec::find_in_path;

#[derive(Clone)]
#[allow(dead_code)]
struct Job {
    id: usize,
    pid: Pid,
    command: String,
    done: bool,
}

thread_local! {
    static JOBS: RefCell<Vec<Job>> = RefCell::new(Vec::new());
}

fn next_job_id(jobs: &[Job]) -> usize {
    let mut id = 1;
    while jobs.iter().any(|j| j.id == id) {
        id += 1;
    }
    id
}

fn check_exited(jobs: &mut [Job]) {
    for job in jobs.iter_mut() {
        if !job.done {
            match waitpid(job.pid, Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::Exited(_, _) | WaitStatus::Signaled(_, _, _)) => {
                    job.done = true;
                }
                _ => {}
            }
        }
    }
}

fn job_marker(index: usize, total: usize) -> &'static str {
    if index == total - 1 {
        "+"
    } else if index == total - 2 {
        "-"
    } else {
        " "
    }
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
            JOBS.with(|jobs| {
                let mut jobs = jobs.borrow_mut();
                let job_id = next_job_id(&jobs);
                jobs.push(Job {
                    id: job_id,
                    pid: child,
                    command: full_command,
                    done: false,
                });
                println!("[{}] {}", job_id, child);
            });
        }
        Ok(ForkResult::Child) => {
            unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL); }
            let _ = unistd::execvp(&c_path, &c_args);
            std::process::exit(1);
        }
        Err(_) => panic!("fork failed"),
    }
}

pub fn reap_jobs() {
    JOBS.with(|jobs| {
        let mut jobs = jobs.borrow_mut();
        check_exited(&mut jobs);

        let len = jobs.len();
        for (i, job) in jobs.iter().enumerate() {
            if job.done {
                let marker = job_marker(i, len);
                println!("[{}]{}  {:<24}{}", job.id, marker, "Done", job.command);
            }
        }

        jobs.retain(|job| !job.done);
    });
}

pub fn print_jobs() {
    JOBS.with(|jobs| {
        let mut jobs = jobs.borrow_mut();
        check_exited(&mut jobs);

        let len = jobs.len();
        for (i, job) in jobs.iter().enumerate() {
            let marker = job_marker(i, len);
            if job.done {
                println!("[{}]{}  {:<24}{}", job.id, marker, "Done", job.command);
            } else {
                println!("[{}]{}  {:<24}{} &", job.id, marker, "Running", job.command);
            }
        }

        jobs.retain(|job| !job.done);
    });
}
