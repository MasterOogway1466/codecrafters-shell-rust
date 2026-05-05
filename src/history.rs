use std::cell::RefCell;

thread_local! {
    static HISTORY: RefCell<Vec<String>> = RefCell::new(Vec::new());
}

pub fn add_to_history(line: &str) {
    HISTORY.with(|h| h.borrow_mut().push(line.to_string()));
}

pub fn print_history() {
    HISTORY.with(|h| {
        for (i, entry) in h.borrow().iter().enumerate() {
            println!("{:>5}  {}", i + 1, entry);
        }
    });
}
