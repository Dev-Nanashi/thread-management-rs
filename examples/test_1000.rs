use uthreads::{thread_create, thread_join};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn main() {
    let num_threads = 1000;
    let mut thread_ids = Vec::new();
    let counter = Arc::new(AtomicUsize::new(0));

    println!("Spawning {} threads...", num_threads);
    for _ in 0..num_threads {
        let counter_clone = Arc::clone(&counter);
        let tid = thread_create(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        thread_ids.push(tid);
    }

    println!("Joining threads...");
    for tid in thread_ids {
        thread_join(tid);
    }

    println!("All threads finished. Counter = {}", counter.load(Ordering::Relaxed));
}
