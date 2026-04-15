//! # Thread Module
//!
//! Defines the Thread Control Block (`Tcb`) and its associated types.
//!
//! Every user-space thread — including the root (main) thread — has exactly one
//! `Tcb` that lives inside the scheduler's `threads` list for the duration of
//! the thread's lifetime.

use std::marker::PhantomData;
use crate::context::{Context, STACK_SIZE};

/// Unique numeric identifier for a user-space thread.
///
/// Thread 0 is always the root (main OS) thread. Subsequent threads are
/// numbered sequentially starting from 1.
pub type ThreadId = usize;

/// Lifecycle state of a user-space thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadState {
    /// Ready to run; sitting in the scheduler's run queue.
    Ready,
    /// Currently executing on the CPU.
    Running,
    /// Waiting for an event (mutex, join), not in the run queue.
    Blocked,
    /// Has returned from its entry function; resources may still be alive.
    Finished,
}

/// Thread Control Block — all per-thread state managed by the scheduler.
pub struct Tcb {
    /// Unique thread identifier.
    pub id: ThreadId,
    /// Current lifecycle state of this thread.
    pub state: ThreadState,
    /// Raw stack memory. Must remain valid for the entire lifetime of the thread.
    ///
    /// The `#[allow(dead_code)]` suppresses the warning because the Vec is held
    /// solely to keep the allocation alive; its pointer is used by `context_stack_ptr`.
    #[allow(dead_code)]
    pub stack: Vec<u8>,
    /// 16-byte-aligned pointer into `stack` that was handed to `makecontext`.
    pub context_stack_ptr: *mut u8,
    /// Usable byte count of the stack starting from `context_stack_ptr`.
    pub context_stack_len: usize,
    /// Saved CPU context (`ucontext_t`) for this thread.
    pub context: Context,
    /// Threads blocked in `thread_join` waiting for this thread to finish.
    pub join_waiting: Vec<ThreadId>,
    /// Entry function to call when the thread is first scheduled.
    ///
    /// Stored as `Option` so that `take()` can move it out exactly once,
    /// preventing double-execution.
    pub func: Option<Box<dyn FnOnce() + Send>>,
    /// Makes `Tcb` (and therefore `Scheduler`) non-`Send` and non-`Sync` at the
    /// type level. The cooperative scheduler assumes a single OS thread — this
    /// marker enforces that at compile time, preventing accidental cross-thread
    /// use of the scheduler which would violate the aliasing invariants upheld
    /// by the `UnsafeCell` in `SCHEDULER`.
    _not_send: PhantomData<*mut ()>,
}

// SAFETY: `Tcb` is only ever accessed from a single OS thread because the
// scheduler is cooperative and single-threaded (stored in a `thread_local`).
// The `_not_send` field keeps `Tcb` non-Send by default; this impl is required
// because `Box<dyn FnOnce() + Send>` pulls in `Send` on the func field, but the
// raw pointer fields (`context_stack_ptr`, `ucontext_t` internals) are not Send.
// The invariant is that `Tcb` values are only ever accessed from the OS thread
// that owns the `SCHEDULER` thread-local.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl Send for Tcb {}


impl Tcb {
    /// Allocate a new thread that will execute `func`.
    ///
    /// Allocates a `STACK_SIZE`-byte stack with enough padding to guarantee a
    /// 16-byte-aligned start address regardless of what the allocator returns.
    pub fn new(id: ThreadId, func: Box<dyn FnOnce() + Send>) -> Self {
        // Allocate STACK_SIZE + 16 extra bytes so we can always carve out a
        // 16-byte-aligned region of at least STACK_SIZE bytes inside the buffer.
        let stack = vec![0u8; STACK_SIZE + 16];

        // Round the raw pointer up to the next 16-byte boundary.
        let raw_ptr = stack.as_ptr() as usize;
        let aligned_ptr = (raw_ptr + 15) & !(15usize);
        let offset = aligned_ptr - raw_ptr;
        let context_stack_ptr = unsafe { stack.as_ptr().add(offset).cast_mut() };

        // By allocating STACK_SIZE + 16 extra bytes and skipping at most 15 bytes
        // for alignment, the usable region is always at least STACK_SIZE bytes.
        let context_stack_len = STACK_SIZE;

        Self {
            id,
            state: ThreadState::Ready,
            stack,
            context_stack_ptr,
            context_stack_len,
            context: Context::new(),
            join_waiting: Vec::new(),
            func: Some(func),
            _not_send: PhantomData,
        }
    }

    /// Create a `Tcb` for the root (main) thread.
    ///
    /// The main thread's stack is managed by the OS, so we do not allocate one
    /// here. `context_stack_ptr` is left null and `func` is `None` because the
    /// main thread's entry point is the program's `main` function.
    pub fn new_main() -> Self {
        Self {
            id: 0,
            state: ThreadState::Running,
            stack: Vec::new(),
            context_stack_ptr: std::ptr::null_mut(),
            context_stack_len: 0,
            context: Context::new(),
            join_waiting: Vec::new(),
            func: None,
            _not_send: PhantomData,
        }
    }
}