use std::sync::Arc;
use uthreads::{mutex_lock, mutex_new, mutex_unlock, thread_create, thread_join, thread_yield};

fn main() {
    let mtx = Arc::new(mutex_new());
    let m1 = Arc::clone(&mtx);
    let t1 = thread_create(move || {
        mutex_lock(&m1);
        println!("T1 locked");
        thread_yield();
        println!("T1 yielding again");
        thread_yield();
        println!("T1 unlocking");
        mutex_unlock(&m1);
    });

    let m2 = Arc::clone(&mtx);
    let t2 = thread_create(move || {
        println!("T2 waiting");
        mutex_lock(&m2);
        println!("T2 locked");
        mutex_unlock(&m2);
    });

    thread_join(t1);
    thread_join(t2);
    println!("OK");
}
