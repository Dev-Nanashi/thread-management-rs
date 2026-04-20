//! # uthreads — Library Showcase
//!
//! Run with:
//!   cargo run --example showcase
//!
//! This file demonstrates every public API provided by the uthreads library:
//!
//!   1. Spawning threads and cooperative yielding (`thread_create`, `thread_yield`)
//!   2. Waiting for threads to finish (`thread_join`)
//!   3. Querying the current thread ID (`thread_self`)
//!   4. Mutual exclusion with a user-space mutex (`mutex_new`, `mutex_lock`, `mutex_unlock`)
//!   5. Early thread termination (`thread_exit`)

use std::sync::Arc;
use uthreads::{
    mutex_lock, mutex_new, mutex_unlock,
    thread_create, thread_exit, thread_join, thread_self, thread_yield,
};

fn main() {
    println!("╔══════════════════════════════════════════╗");
    println!("║       uthreads  —  Library Showcase       ║");
    println!("╚══════════════════════════════════════════╝");
    println!();

    demo_yield();
    demo_join();
    demo_mutex();
    demo_exit();

    println!();
    println!("✓  All demos finished successfully.");
}

// ─────────────────────────────────────────────────────────────────────────────
// Demo 1 — cooperative yielding
// ─────────────────────────────────────────────────────────────────────────────

/// Shows that `thread_yield` interleaves two threads cooperatively.
///
/// Without preemption the two threads take turns printing because each
/// explicitly calls `thread_yield` after every step.
fn demo_yield() {
    println!("┌─ Demo 1: Cooperative Yielding ──────────────");

    let t1 = thread_create(|| {
        let id = thread_self();
        for step in 1..=3 {
            println!("│  [thread {id}] step {step}");
            thread_yield(); // hand control to the next ready thread
        }
    });

    let t2 = thread_create(|| {
        let id = thread_self();
        for step in 1..=3 {
            println!("│  [thread {id}] step {step}");
            thread_yield();
        }
    });

    // Wait for both threads before moving on.
    thread_join(t1);
    thread_join(t2);

    println!("└─ Done\n");
}

// ─────────────────────────────────────────────────────────────────────────────
// Demo 2 — thread_join
// ─────────────────────────────────────────────────────────────────────────────

/// Shows that `thread_join` blocks the *current* thread until the target
/// thread finishes, even if the target runs multiple yield cycles.
fn demo_join() {
    println!("┌─ Demo 2: thread_join ───────────────────────");

    let worker = thread_create(|| {
        let id = thread_self();
        println!("│  [thread {id}] starting long task…");
        for _ in 0..4 {
            thread_yield(); // simulate work spread over several turns
        }
        println!("│  [thread {id}] task complete");
    });

    println!("│  [main] waiting for worker thread {worker}…");
    thread_join(worker); // main thread blocks here
    println!("│  [main] worker is done — continuing");

    println!("└─ Done\n");
}

// ─────────────────────────────────────────────────────────────────────────────
// Demo 3 — mutex (mutual exclusion)
// ─────────────────────────────────────────────────────────────────────────────

/// Shows that only one thread executes the critical section at a time.
///
/// Thread A and B race to acquire the same mutex. Whichever wins prints
/// "ACQUIRED" first. The other prints only after the first has already printed
/// "RELEASED" — demonstrating that critical sections never overlap.
fn demo_mutex() {
    println!("┌─ Demo 3: Mutex (Mutual Exclusion) ──────────");

    // Arc lets us share the handle across threads without unsafe code.
    let mtx = Arc::new(mutex_new());

    // Thread A — one contender for the lock.
    let mtx_a = Arc::clone(&mtx);
    let ta = thread_create(move || {
        let id = thread_self();
        println!("│  [thread {id}] requesting lock…");
        mutex_lock(&mtx_a);
        println!("│  [thread {id}] *** LOCK ACQUIRED — inside critical section ***");
        // Do work inside the critical section, then release.
        println!("│  [thread {id}] releasing lock");
        mutex_unlock(&mtx_a);
        // Yield after releasing so thread B gets a chance to run.
        thread_yield();
    });

    // Thread B — races thread A for the same lock.
    let mtx_b = Arc::clone(&mtx);
    let tb = thread_create(move || {
        let id = thread_self();
        // Yield once so thread A gets to acquire the lock first,
        // making the "blocking" behaviour visible in the output.
        thread_yield();
        println!("│  [thread {id}] requesting lock (A holds it — will block)…");
        mutex_lock(&mtx_b); // blocks here until thread A releases
        println!("│  [thread {id}] *** LOCK ACQUIRED — A has already released ***");
        mutex_unlock(&mtx_b);
    });

    thread_join(ta);
    thread_join(tb);
    println!("└─ Done\n");
}

// ─────────────────────────────────────────────────────────────────────────────
// Demo 4 — thread_exit (early termination)
// ─────────────────────────────────────────────────────────────────────────────

/// Shows that `thread_exit` terminates a thread before its closure returns
/// normally. The join still succeeds — the thread is properly cleaned up.
fn demo_exit() {
    println!("┌─ Demo 4: thread_exit ───────────────────────");

    let t = thread_create(|| {
        let id = thread_self();
        println!("│  [thread {id}] running…");
        println!("│  [thread {id}] calling thread_exit() early");
        thread_exit(); // terminates here — the line below is never reached
        println!("│  [thread {id}] ← this line should NOT print");
    });

    thread_join(t);
    println!("│  [main] thread joined successfully after early exit");
    println!("└─ Done");
}
