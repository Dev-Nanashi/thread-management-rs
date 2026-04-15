//! # Scheduler Module
//!
//! Round-robin cooperative scheduler for user-space threads.
//!
//! The scheduler lives in a `thread_local` [`UnsafeCell`] (`SCHEDULER`) so
//! that all library functions can access it without passing a reference around.
//! Because the scheduler is cooperative and single-threaded, no actual data
//! races can occur — the `UnsafeCell` is safe in this context.

use std::collections::VecDeque;
use std::cell::UnsafeCell;
use crate::thread::{Tcb, ThreadId, ThreadState};
use crate::context::{make_context, swap_context};

/// Central scheduler that owns all `Tcb`s and drives context switching.
pub struct Scheduler {
    /// All known threads, including the main thread (id 0) and finished ones.
    ///
    /// `clippy::vec_box` is suppressed because `Box<Tcb>` gives each TCB a
    /// stable address in memory, which is required for the raw `context` pointers
    /// passed to `swapcontext`.
    #[allow(clippy::vec_box)]
    pub threads: Vec<Box<Tcb>>,
    /// FIFO run queue of thread IDs that are in the `Ready` state.
    pub ready_queue: VecDeque<ThreadId>,
    /// ID of the thread currently executing.
    pub current: ThreadId,
    /// Monotonically increasing counter used to assign IDs to new threads.
    pub next_id: ThreadId,
}

impl Scheduler {
    /// Initialize the scheduler with just the main thread (id 0) as current.
    pub fn new() -> Self {
        let main_tcb = Box::new(Tcb::new_main());
        Self {
            threads: vec![main_tcb],
            ready_queue: VecDeque::new(),
            current: 0,
            next_id: 1,
        }
    }

    /// Return an immutable reference to the `Tcb` with the given `id`, if any.
    pub fn get_thread(&self, id: ThreadId) -> Option<&Tcb> {
        self.threads.iter().find(|t| t.id == id).map(|v| &**v)
    }

    /// Return a mutable reference to the `Tcb` with the given `id`, if any.
    pub fn get_thread_mut(&mut self, id: ThreadId) -> Option<&mut Tcb> {
        self.threads.iter_mut().find(|t| t.id == id).map(|v| &mut **v)
    }

    /// Register a new thread and enqueue it for scheduling.
    ///
    /// Sets up the thread's entry context via [`make_context`] so that the
    /// first `swap_context` into this thread will call [`thread_entry`].
    /// The `uc_link` is pointed at the main thread's context so that an
    /// accidental return from `thread_entry` falls through to the scheduler
    /// rather than calling `exit()` on the whole process.
    pub fn add_thread(&mut self, mut tcb: Box<Tcb>) {
        let id = tcb.id;

        // Obtain a raw pointer to the main thread's ucontext_t to use as uc_link.
        // SAFETY: The main thread (id 0) always exists for the lifetime of the
        // scheduler, so this pointer remains valid as long as the scheduler is alive.
        let main_ctx_ptr = self.threads
            .iter_mut()
            .find(|t| t.id == 0)
            .map(|t| &raw mut t.context.uctx)
            .expect("main thread (id 0) must always exist");

        unsafe {
            make_context(
                &mut tcb.context,
                tcb.context_stack_ptr,
                tcb.context_stack_len,
                thread_entry,
                main_ctx_ptr,
            );
        }
        self.threads.push(tcb);
        self.ready_queue.push_back(id);
    }

    /// Pick the next `Ready` thread from the queue and switch into it.
    ///
    /// Stale queue entries (threads that are no longer `Ready`) are silently
    /// discarded. If the queue is empty and the current thread is the only
    /// remaining one, the scheduler either continues running it or exits.
    pub fn schedule(&mut self) {
        // Walk the queue until we find a thread that is actually Ready.
        let next_id = loop {
            if let Some(id) = self.ready_queue.pop_front() {
                if let Some(t) = self.get_thread(id) {
                    if t.state == ThreadState::Ready {
                        break id; // Found a runnable thread.
                    }
                    // Discard stale queue entry (thread may be Blocked/Finished).
                }
            } else {
                // Queue empty — decide what to do based on the current thread's state.
                let current_state = self.get_thread(self.current).map(|t| t.state.clone());
                if current_state == Some(ThreadState::Blocked) {
                    panic!("Deadlock detected: all threads are blocked!");
                } else if current_state == Some(ThreadState::Finished) {
                    // Check if any threads are still blocked — that would be a deadlock.
                    let blocked_count = self.threads.iter()
                        .filter(|t| t.state == ThreadState::Blocked)
                        .count();
                    assert!(
                        blocked_count == 0,
                        "Deadlock detected: process is exiting but {blocked_count} thread(s) are \
                         still blocked (waiting on a mutex or join that will never be resolved)."
                    );
                    // All threads are done; exit cleanly.
                    std::process::exit(0);
                }
                // Current thread is still runnable — keep executing it.
                return;
            }
        };

        let current_id = self.current;

        // Swapping a thread with itself is a no-op and would create aliased
        // mutable pointers below, which is undefined behaviour.
        if current_id == next_id {
            self.ready_queue.push_front(next_id); // Put it back for future rounds.
            return;
        }

        self.current = next_id;

        if let Some(t) = self.get_thread_mut(next_id) {
            t.state = ThreadState::Running;
        }

        // Obtain raw pointers to both contexts in a single pass over `self.threads`
        // to avoid holding two &mut references simultaneously.
        //
        // SAFETY: `current_id != next_id` is guaranteed above, so `from` and
        // `to` always point to distinct memory locations.
        let mut from_ptr = None::<*mut _>;
        let mut to_ptr   = None::<*mut _>;
        for t in &mut self.threads {
            if t.id == current_id { from_ptr = Some(&raw mut t.context); }
            if t.id == next_id    { to_ptr   = Some(&raw mut t.context); }
            if from_ptr.is_some() && to_ptr.is_some() { break; }
        }

        if let (Some(from), Some(to)) = (from_ptr, to_ptr) {
            unsafe {
                swap_context(&mut *from, &mut *to);
            }
        }
    }

    /// Voluntarily give up the CPU.
    ///
    /// Transitions the current thread from `Running` → `Ready`, re-enqueues it
    /// at the back of the run queue, then calls `schedule()` to switch to the
    /// next ready thread.
    pub fn yield_current(&mut self) {
        let current_id = self.current;
        if let Some(t) = self.get_thread_mut(current_id) {
            if t.state == ThreadState::Running {
                t.state = ThreadState::Ready;
                self.ready_queue.push_back(current_id);
            }
        }
        self.schedule();
    }

    /// Mark the current thread as finished and wake any threads join-waiting on it.
    ///
    /// All threads in the current thread's `join_waiting` list are transitioned
    /// from `Blocked` → `Ready` and enqueued. Then the scheduler switches to
    /// the next available thread.
    ///
    /// Note: the current thread's `Tcb` is intentionally left in `self.threads`
    /// even after it is marked `Finished`. Its stack memory must remain valid
    /// until the context switch (inside `schedule`) has fully transferred
    /// control to the next thread. Pruning is deferred to the *next* call to
    /// `finish_current` or `add_thread` so that we never free a stack that is
    /// still live beneath us.
    pub fn finish_current(&mut self) {
        let current_id = self.current;

        // Opportunistically reclaim TCBs from *previous* finished threads.
        // These are safe to drop because they are not the active stack.
        self.threads.retain(|t| t.id == current_id || t.state != ThreadState::Finished);

        // Collect waiters before mutably borrowing to change state.
        let waiters: Vec<ThreadId> = self.get_thread(current_id)
            .map(|t| t.join_waiting.clone())
            .unwrap_or_default();

        // Unblock every thread that was waiting for this one to finish.
        for waiter_id in waiters {
            if let Some(wt) = self.get_thread_mut(waiter_id) {
                if wt.state == ThreadState::Blocked {
                    wt.state = ThreadState::Ready;
                    let wid = wt.id;
                    self.ready_queue.push_back(wid);
                }
            }
        }

        // Mark finished *after* waking waiters (avoids double-borrow).
        if let Some(t) = self.get_thread_mut(current_id) {
            t.state = ThreadState::Finished;
        }

        // Transfer control to the next ready thread. After this swap_context
        // call we are running on a different stack — code below does not execute
        // until (if ever) this thread is resumed, which will not happen because
        // it is now Finished.
        self.schedule();
    }

    /// Block the current thread and yield to the scheduler.
    ///
    /// The caller is responsible for enqueuing the thread again (e.g. via
    /// `unblock`) when the blocking condition is resolved.
    pub fn block_current(&mut self) {
        let current_id = self.current;
        if let Some(t) = self.get_thread_mut(current_id) {
            t.state = ThreadState::Blocked;
        }
        self.schedule();
    }

    /// Transition a `Blocked` thread back to `Ready` and enqueue it.
    ///
    /// Has no effect if the thread is not in the `Blocked` state.
    pub fn unblock(&mut self, id: ThreadId) {
        if let Some(t) = self.get_thread_mut(id) {
            if t.state == ThreadState::Blocked {
                t.state = ThreadState::Ready;
                self.ready_queue.push_back(id);
            }
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// C-ABI entry point invoked when a new thread is first switched into.
///
/// Retrieves the closure stored in the current thread's `Tcb`, runs it, then
/// calls `finish_current` to perform cleanup and hand control back to the
/// scheduler.
pub extern "C" fn thread_entry() {
    // Take the closure out of the TCB so it can be called by value (FnOnce).
    let func = SCHEDULER.with(|s| {
        let sched = unsafe { &mut *s.get() };
        let current_id = sched.current;
        sched.get_thread_mut(current_id)
            .and_then(|t| t.func.take())
    });

    if let Some(f) = func {
        f();
    }

    // The thread has returned — mark it finished and switch away.
    SCHEDULER.with(|s| {
        let sched = unsafe { &mut *s.get() };
        sched.finish_current();
    });
}

// Per-OS-thread singleton scheduler instance.
// Using `thread_local!` means each OS thread has its own scheduler, which
// prevents data races without needing a `Mutex`. Since the scheduler is
// cooperative (only one user-space thread runs at a time on each OS thread),
// the `UnsafeCell` is safe to dereference from library API functions.
thread_local! {
    pub static SCHEDULER: UnsafeCell<Scheduler> = UnsafeCell::new(Scheduler::new());
}