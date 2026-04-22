# uthreads

A minimal **cooperative user-space threading library** written in Rust.

Threads are scheduled round-robin and must voluntarily **yield** control — there is no preemption. Context switching is implemented with POSIX `ucontext` APIs (`getcontext` / `makecontext` / `swapcontext`).

> **Platform:** Linux / macOS only (requires POSIX `ucontext`). Does not compile on Windows.

---

## Public API

| Function | Description |
|---|---|
| `thread_create(fn)` | Spawn a new cooperative thread; returns its `ThreadId` |
| `thread_self()` | Return the `ThreadId` of the calling thread |
| `thread_yield()` | Voluntarily hand the CPU to the next ready thread |
| `thread_join(id)` | Block until thread `id` finishes |
| `thread_exit()` | Terminate the calling thread immediately |
| `mutex_new()` | Create a new cooperative mutex; returns a `MutexHandle` |
| `mutex_lock(&handle)` | Acquire the mutex, blocking if another thread holds it |
| `mutex_unlock(&handle)` | Release the mutex and wake the next waiter |

---

## Running the showcase

```bash
cargo run --example showcase
```

Expected output (thread IDs may vary):

```
╔══════════════════════════════════════════╗
║       uthreads  —  Library Showcase       ║
╚══════════════════════════════════════════╝

┌─ Demo 1: Cooperative Yielding ──────────────
│  [thread 1] step 1
│  [thread 2] step 1
│  [thread 1] step 2
│  [thread 2] step 2
│  [thread 1] step 3
│  [thread 2] step 3
└─ Done

┌─ Demo 2: thread_join ───────────────────────
│  [main] waiting for worker thread 3…
│  [thread 3] starting long task…
│  [thread 3] task complete
│  [main] worker is done — continuing
└─ Done

┌─ Demo 3: Mutex (Mutual Exclusion) ──────────
│  [thread 4] requesting lock…
│  [thread 4] *** LOCK ACQUIRED — inside critical section ***
│  [thread 5] requesting lock (will block if A holds it)…
│  [thread 4] releasing lock
│  [thread 5] *** LOCK ACQUIRED — A has already released ***
└─ Done

┌─ Demo 4: thread_exit ───────────────────────
│  [thread 6] running…
│  [thread 6] calling thread_exit() early
│  [main] thread joined successfully after early exit
└─ Done

✓  All demos finished successfully.
```

---

## Using it in your own code

Add this crate as a path dependency in your project's `Cargo.toml`:

```toml
[dependencies]
uthreads = { path = "../Thread_Library" }
```

Then use the public API:

```rust
use uthreads::{thread_create, thread_join, thread_yield};

fn main() {
    let t = thread_create(|| {
        println!("Hello from thread {}!", uthreads::thread_self());
        thread_yield();
        println!("Back after yield.");
    });
    thread_join(t);
}
```

---

## Architecture

```
src/
├── lib.rs        — Public API (thread_create, thread_join, mutex_*, …)
├── scheduler.rs  — Round-robin cooperative scheduler + context switching
├── thread.rs     — Thread Control Block (Tcb) and ThreadState enum
├── context.rs    — Thin wrapper around POSIX ucontext (make/swap context)
└── mutex.rs      — Internal cooperative mutex state (locked, owner, wait queue)

examples/
└── showcase.rs   — Runnable demonstration of every API function
```

## How it works

1. Every thread has a **Thread Control Block** (`Tcb`) holding its saved CPU context, stack, and state.
2. The **scheduler** lives in a `thread_local` and maintains a FIFO ready queue.
3. `thread_yield` / `thread_join` / `mutex_lock` call into the scheduler, which uses `swapcontext` to switch to the next ready thread.
4. Mutexes are cooperative — a blocked thread is marked `Blocked` and removed from the run queue until the current owner calls `mutex_unlock`.

