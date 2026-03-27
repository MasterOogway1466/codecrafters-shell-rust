#[allow(unused_imports)]
use std::io::{self, Write};
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::ffi::CString;
use nix::sys::wait::waitpid;
use nix::unistd::{self, ForkResult};

fn main() {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();
        let mut input: String = "".to_string();
        io::stdin().read_line(&mut input).unwrap();
        input.pop();
        let command: Vec<&str> = input.split(" ").collect();
        if command[0] == "exit" {
            break;
        }
        eval_command(command[0], command[1..command.len()].to_vec());
    }
}

fn find_in_path(name: &str) -> Option<String> {
    let path_env = env::var("PATH").ok()?;
    for dir in path_env.split(':') {
        let full_path = format!("{}/{}", dir, name);
        let path = Path::new(&full_path);
        if path.exists() {
            if let Ok(metadata) = fs::metadata(path) {
                if metadata.permissions().mode() & 0o111 != 0 {
                    return Some(full_path);
                }
            }
        }
    }
    None
}

fn eval_command(command: &str, args: Vec<&str>) {
    let known_commands = ["exit", "echo", "type"];

    match command {
        "echo" => {
            println!("{}", args.join(" "));
        }
        "type" => {
            let target = args[0];
            if known_commands.contains(&target) {
                println!("{} is a shell builtin", target);
            } else {
                match find_in_path(target) {
                    Some(path) => println!("{} is {}", target, path),
                    None => println!("{}: not found", target),
                }
            }
        }
        _ => {
            match find_in_path(command) {
                Some(path) => {
                    let mut full_args: Vec<&str> = vec![command];
                    full_args.extend(args.iter());
                    let c_path = CString::new(path).unwrap();
                    let c_args: Vec<CString> = full_args
                        .iter()
                        .map(|&s| CString::new(s).unwrap())
                        .collect();
                    match unsafe { unistd::fork() } {
                        Ok(ForkResult::Parent { child }) => {
                            let _ = waitpid(child, None);
                        }
                        Ok(ForkResult::Child) => {
                            let _ = unistd::execvp(&c_path, &c_args);
                            std::process::exit(1);
                        }
                        Err(_) => {
                            panic!("fork failed");
                        }
                    }
                }
                None => println!("{}: command not found", command),
            }
        }
    }
}