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

---

## Web UI Dashboard (WIP)

We are currently building a real-time visualization dashboard to monitor threads and scheduling events.

**Architecture:**
- **Rust Core**: Unmodified, produces standard output logs.
- **Node.js Bridge (`ui-bridge/`)**: Spawns the Rust process, parses its output, and broadcasts state via WebSocket.
- **Next.js Dashboard (`dashboard/`)**: The frontend UI for visualization.

**Phase 2: Node.js Data Bridge**
The `ui-bridge/bridge.js` acts as the middleware. It spawns the Rust binary (`cargo run --example showcase`), captures `stdout`, parses the terminal strings via regex into structured JSON, and broadcasts them over Socket.io on port 3001.

**Phase 3: State Management**
The Next.js app maintains local React state variables (`activeThread`, `readyQueue`, `blockedQueue`, `terminalLogs`) that mirror the internal Rust POSIX ucontext scheduler. It listens to the `os_event` Socket.io channel on `http://localhost:3001`, parsing and reducing incoming structured state updates directly into the UI state in real-time.

**Phase 4: Frontend UI**
The Dashboard now features a highly polished, responsive dark-mode UI powered by Tailwind CSS. Visual segregation separates the **CPU Execution** zone (which glows when active), a horizontal flex-based **Ready Queue**, a highlighted red **Blocked Queue** for threads halted on cooperative mutex execution, and a scrolling live **Terminal Stream** directly mirroring the Rust backend.

**Phase 5 & 6: Simulation & High-Volume Animation Mode**
A frontend-only mock data generator was integrated to physically simulate the internal logic of the POSIX backend. Recently upgraded to a randomized, algorithmic load tester supporting 15+ concurrent threads. Utilizing `framer-motion`, the dynamic UI renders smooth, cinematic translations highlighting exact threading constraints (Yielding, Wait-Lists, Context Switching, Mutual Exclusivity) ensuring robust testing capability entirely on native Windows.

**How to run the UI (Development):**

1. **Start the Node.js Bridge:**
   ```bash
   cd ui-bridge
   node bridge.js
   ```

2. **Start the Next.js Dashboard:**
   ```bash
   cd dashboard
   npm run dev
   ```

