use std::cell::RefCell;

thread_local! {
    static HISTORY: RefCell<Vec<String>> = RefCell::new(Vec::new());
}

pub fn add_to_history(line: &str) {
    HISTORY.with(|h| h.borrow_mut().push(line.to_string()));
}

pub fn print_history(n: Option<usize>) {
    HISTORY.with(|h| {
        let history = h.borrow();
        let total = history.len();
        let start = match n {
            Some(count) => total.saturating_sub(count),
            None => 0,
        };
        for (i, entry) in history.iter().enumerate().skip(start) {
            println!("{:>5}  {}", i + 1, entry);
        }
    });
}
