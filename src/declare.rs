use std::collections::HashMap;
use std::cell::RefCell;

thread_local! {
    pub static VARIABLES: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

pub fn handle_declare(args: &[String]) {
    match args.first().map(|s| s.as_str()) {
        Some("-p") => {
            if let Some(name) = args.get(1) {
                VARIABLES.with(|v| {
                    match v.borrow().get(name.as_str()) {
                        Some(val) => println!("declare -- {}=\"{}\"", name, val),
                        None => eprintln!("declare: {}: not found", name),
                    }
                });
            }
        }
        Some(assignment) if assignment.contains('=') => {
            if let Some((name, value)) = assignment.split_once('=') {
                VARIABLES.with(|v| {
                    v.borrow_mut().insert(name.to_string(), value.to_string());
                });
            }
        }
        _ => {}
    }
}
