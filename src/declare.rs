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
                        Some(val) => println!("declare -x {}=\"{}\"", name, val),
                        None => eprintln!("declare: {}: not found", name),
                    }
                });
            }
        }
        _ => {}
    }
}
