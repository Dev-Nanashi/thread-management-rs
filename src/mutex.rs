//! # Mutex Module
//!
//! Internal representation of a cooperative user-space mutex.
//!
//! This mutex is **not** backed by an OS primitive — it is entirely managed by
//! the cooperative scheduler. Threads that cannot acquire the lock are placed
//! in `wait_queue` and kept in the `Blocked` state until the owner releases it.

use std::collections::VecDeque;
use crate::thread::ThreadId;

/// Internal state of a cooperative mutex.
///
/// Instances are owned by `THREAD_MUTEXES` in `lib.rs` and are never exposed
/// directly to library users. Public API is through `MutexHandle`.
pub struct Mutex {
    /// Whether the mutex is currently held by any thread.
    pub locked: bool,
    /// The thread currently holding the lock, or `None` if unlocked.
    pub owner: Option<ThreadId>,
    /// FIFO queue of threads waiting to acquire this mutex.
    ///
    /// On unlock, the front of the queue is granted the lock and unblocked by
    /// the scheduler (`lib::mutex_unlock`).
    pub wait_queue: VecDeque<ThreadId>,
}

impl Mutex {
    /// Create a new, unlocked mutex with an empty wait queue.
    pub const fn new() -> Self {
        Self {
            locked: false,
            owner: None,
            wait_queue: VecDeque::new(),
        }
    }
}