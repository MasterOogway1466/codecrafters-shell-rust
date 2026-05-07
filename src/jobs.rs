use std::cell::RefCell;
use std::ffi::CString;

use nix::libc;
use nix::sys::wait::{WaitPidFlag, waitpid};
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
            let job_id = JOBS.with(|jobs| {
                let jobs = jobs.borrow();
                let mut id = 1;
                loop {
                    if !jobs.iter().any(|j| j.id == id) {
                        break id;
                    }
                    id += 1;
                }
            });
            JOBS.with(|jobs| {
                jobs.borrow_mut().push(Job {
                    id: job_id,
                    pid: child,
                    command: full_command,
                    done: false,
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

pub fn reap_jobs() {
    JOBS.with(|jobs| {
        let mut jobs = jobs.borrow_mut();

        // Check each job for completion
        for job in jobs.iter_mut() {
            if !job.done {
                match waitpid(job.pid, Some(WaitPidFlag::WNOHANG)) {
                    Ok(nix::sys::wait::WaitStatus::Exited(_, _)) => {
                        job.done = true;
                    }
                    Ok(nix::sys::wait::WaitStatus::Signaled(_, _, _)) => {
                        job.done = true;
                    }
                    _ => {}
                }
            }
        }

        // Print Done lines for newly completed jobs
        let len = jobs.len();
        for (i, job) in jobs.iter().enumerate() {
            if job.done {
                let marker = if i == len - 1 {
                    "+"
                } else if i == len - 2 {
                    "-"
                } else {
                    " "
                };
                println!("[{}]{}  {:<24}{}", job.id, marker, "Done", job.command);
            }
        }

        // Remove done jobs
        jobs.retain(|job| !job.done);
    });
}

pub fn print_jobs() {
    JOBS.with(|jobs| {
        // First reap completed jobs
        {
            let mut jobs = jobs.borrow_mut();
            for job in jobs.iter_mut() {
                if !job.done {
                    match waitpid(job.pid, Some(WaitPidFlag::WNOHANG)) {
                        Ok(nix::sys::wait::WaitStatus::Exited(_, _)) => {
                            job.done = true;
                        }
                        Ok(nix::sys::wait::WaitStatus::Signaled(_, _, _)) => {
                            job.done = true;
                        }
                        _ => {}
                    }
                }
            }
        }

        // Print all jobs with correct markers
        let jobs_ref = jobs.borrow();
        let len = jobs_ref.len();
        for (i, job) in jobs_ref.iter().enumerate() {
            let marker = if i == len - 1 {
                "+"
            } else if i == len - 2 {
                "-"
            } else {
                " "
            };
            if job.done {
                println!("[{}]{}  {:<24}{}", job.id, marker, "Done", job.command);
            } else {
                println!("[{}]{}  {:<24}{} &", job.id, marker, "Running", job.command);
            }
        }

        // Remove done jobs
        drop(jobs_ref);
        jobs.borrow_mut().retain(|job| !job.done);
    });
}
