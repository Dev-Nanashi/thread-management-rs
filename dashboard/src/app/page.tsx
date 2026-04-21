'use client';

import { useEffect, useState, useCallback, useRef } from 'react';
import { io, Socket } from 'socket.io-client';
import { motion } from 'framer-motion';

// Define the shape of incoming OS events emitted by our Node bridge
interface OsEvent {
  type: 'CPU_ACTIVE' | 'BLOCKED' | 'MUTEX_ACQUIRED' | 'EXITED' | 'LOG';
  text: string;
  threadId: string | null;
}

export default function Dashboard() {
  // === UI State Management ===
  const [activeThread, setActiveThread] = useState<string | null>(null);
  const [readyQueue, setReadyQueue] = useState<string[]>([]);
  const [blockedQueue, setBlockedQueue] = useState<string[]>([]);
  const [terminalLogs, setTerminalLogs] = useState<string[]>([]);
  
  // === Simulation Mode ===
  const [isSimulating, setIsSimulating] = useState(false);
  const simStateRef = useRef({ active: null as string | null, ready: [] as string[], blocked: [] as string[] });

  // Centralized dispatcher for state reduction (Used by actual WebSockets)
  const dispatchOsEvent = useCallback((event: OsEvent) => {
    setTerminalLogs(prev => {
      const newLogs = [...prev, event.text];
      return newLogs.slice(-20);
    });

    if (!event.threadId) return;

    switch (event.type) {
      case 'CPU_ACTIVE':
      case 'MUTEX_ACQUIRED':
        setActiveThread(event.threadId);
        setReadyQueue(prev => prev.filter(id => id !== event.threadId));
        setBlockedQueue(prev => prev.filter(id => id !== event.threadId));
        break;
        
      case 'BLOCKED':
        setBlockedQueue(prev => {
          if (!prev.includes(event.threadId!)) return [...prev, event.threadId!];
          return prev;
        });
        setActiveThread(prev => (prev === event.threadId ? null : prev));
        setReadyQueue(prev => prev.filter(id => id !== event.threadId));
        break;
        
      case 'EXITED':
        setActiveThread(prev => (prev === event.threadId ? null : prev));
        setReadyQueue(prev => prev.filter(id => id !== event.threadId));
        setBlockedQueue(prev => prev.filter(id => id !== event.threadId));
        break;
        
      default:
        break;
    }
  }, []);

  useEffect(() => {
    // === WebSocket Connection Setup ===
    const socket: Socket = io('http://localhost:3001');

    socket.on('connect', () => {
      console.log('Connected to Node Bridge on port 3001!');
    });

    socket.on('os_event', dispatchOsEvent);

    return () => {
      socket.disconnect();
    };
  }, [dispatchOsEvent]);

  // === Simulation Mode (High-Volume Load Tester) ===
  // Because the POSIX backend is fundamentally unsupported natively on Windows, 
  // this dynamic algorithm tests our React scheduler UI against heavy thread loads.
  useEffect(() => {
    if (!isSimulating) return;

    // Initialization: Seed UI with 15 concurrent threads to demonstrate scalable layout & animations
    simStateRef.current = {
      active: null,
      ready: Array.from({ length: 15 }, (_, i) => String(i + 1)),
      blocked: []
    };
    
    setActiveThread(null);
    setBlockedQueue([]);
    setReadyQueue([...simStateRef.current.ready]);
    setTerminalLogs(['[Simulator] Initialized high-volume load test with 15 threads.']);

    const interval = setInterval(() => {
      let state = simStateRef.current;
      let newLogs: string[] = [];

      // Phase A: Unblock (Simulating Mutex Release)
      // Randomly pick 0 to 2 threads from blocked queue and push to back of ready queue
      if (state.blocked.length > 0) {
        const numToUnblock = Math.floor(Math.random() * 3);
        const actualToUnblock = Math.min(numToUnblock, state.blocked.length);
        for (let i = 0; i < actualToUnblock; i++) {
          const idx = Math.floor(Math.random() * state.blocked.length);
          const threadId = state.blocked[idx];
          state.blocked.splice(idx, 1);
          state.ready.push(threadId);
          newLogs.push(`[thread ${threadId}] mutex unlocked -> ready_queue`);
        }
      }

      // Phase B: Context Switch (Simulating Yields, Blocks, and Exits)
      let needsNewActive = false;
      if (state.active) {
        const r = Math.random();
        const currentActive = state.active;
        if (r < 0.6) {
          // 60% Chance: Thread Yields (Round Robin)
          state.active = null;
          state.ready.push(currentActive);
          needsNewActive = true;
          newLogs.push(`[thread ${currentActive}] thread_yield() -> ready_queue`);
        } else if (r < 0.9) {
          // 30% Chance: Thread Blocks (Mutex Lock Wait)
          state.active = null;
          state.blocked.push(currentActive);
          needsNewActive = true;
          newLogs.push(`[thread ${currentActive}] mutex_lock() stalled -> blocked_queue`);
        } else {
          // 10% Chance: Thread Exits (Terminates)
          state.active = null;
          needsNewActive = true;
          newLogs.push(`[thread ${currentActive}] thread_exit() -> terminated`);
        }
      } else {
        needsNewActive = true; // CPU is idle, grab next thread
      }

      // If CPU needs work, take the first thread from Ready Queue FIFO style
      if (needsNewActive && state.ready.length > 0) {
        const nextActive = state.ready.shift()!;
        state.active = nextActive;
        newLogs.push(`[thread ${nextActive}] swapped into CPU...`);
      } else if (needsNewActive && state.ready.length === 0 && state.blocked.length === 0) {
        // Simulation exhausted
        newLogs.push(`[Simulator] All 15 threads exited. Halting load test.`);
        setIsSimulating(false);
      }

      // Flush our rigorously mutually-exclusive internal simulation ref to the actual React UI States
      setActiveThread(state.active);
      setReadyQueue([...state.ready]);
      setBlockedQueue([...state.blocked]);
      if (newLogs.length > 0) {
        setTerminalLogs(prev => {
          const nextLogs = [...prev, ...newLogs];
          return nextLogs.slice(-20);
        });
      }

    }, 800);

    return () => clearInterval(interval);
  }, [isSimulating]);

  return (
    <div className="bg-[#0B0F19] text-slate-300 min-h-screen p-8 font-sans overflow-x-hidden">
      <div className="flex flex-col md:flex-row justify-between items-center mb-8 border-b border-slate-800/50 pb-6">
        <h1 className="text-4xl font-extrabold text-slate-100 tracking-tight mb-4 md:mb-0">uthreads<span className="text-slate-500 font-light ml-2">Visualizer</span></h1>
        <button 
          onClick={() => setIsSimulating(!isSimulating)}
          className={`flex items-center gap-2 px-5 py-2.5 text-sm font-semibold tracking-wide rounded-lg transition-all duration-300 ${
            isSimulating 
              ? 'bg-rose-500/10 hover:bg-rose-500/20 text-rose-400 border border-rose-500/30' 
              : 'bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-400 border border-emerald-500/30 shadow-[0_0_15px_rgba(16,185,129,0.1)]'
          }`}
        >
          {isSimulating ? '■ STOP SIMULATION' : '▶ START SIMULATION'}
        </button>
      </div>
      
      <div className="flex flex-col lg:flex-row gap-6 mb-8">
        
        {/* === CPU Execution Zone === */}
        {/* We use framer-motion's 'layout' prop here and below so that threads glide cleanly between queues visually upon state change. */}
        <div className={`flex flex-col justify-center items-center p-8 rounded-2xl transition-all duration-500 w-full lg:w-1/3 backdrop-blur-md ${activeThread ? 'border border-emerald-500/50 bg-emerald-950/20 shadow-[0_0_30px_rgba(16,185,129,0.15)]' : 'border border-slate-800/50 bg-slate-900/50 shadow-sm'}`}>
          <h2 className={`text-sm font-bold uppercase tracking-widest mb-6 ${activeThread ? 'text-emerald-400/80' : 'text-slate-500'}`}>CPU EXECUTION</h2>
          <div className="h-28 flex items-center justify-center">
            {activeThread ? (
              <motion.div 
                layout
                initial={{ opacity: 0, scale: 0.8, y: 10 }}
                animate={{ opacity: 1, scale: 1, y: 0 }}
                whileHover={{ scale: 1.05 }}
                key={`t-${activeThread}`}
                className="w-24 h-24 flex items-center justify-center rounded-xl bg-gradient-to-br from-emerald-500/20 to-emerald-900/40 border border-emerald-500/40 text-5xl font-black text-emerald-300 shadow-[inset_0_0_15px_rgba(16,185,129,0.2)]"
              >
                T{activeThread}
              </motion.div>
            ) : (
              <div className="text-5xl font-black text-slate-800/80">IDLE</div>
            )}
          </div>
        </div>

        <div className="flex flex-col gap-6 w-full lg:w-2/3">
          {/* === Ready Queue Zone === */}
          {/* Layout modified for High-Volume wrapping using flex-wrap */}
          <div className="p-6 bg-slate-900/50 backdrop-blur-md border border-slate-800/50 rounded-2xl shadow-sm min-h-[13rem] flex flex-col">
            <h2 className="text-sm font-bold text-slate-400 mb-4 uppercase tracking-widest border-b border-slate-800/50 pb-3">Ready Queue</h2>
            <div className="flex flex-wrap gap-3 py-2 flex-1 items-start content-start">
              {readyQueue.length > 0 ? (
                readyQueue.map((id) => (
                  <motion.div 
                    layout
                    initial={{ opacity: 0, scale: 0.8 }}
                    animate={{ opacity: 1, scale: 1 }}
                    whileHover={{ scale: 1.05 }}
                    key={`t-${id}`} 
                    className="w-14 h-14 flex items-center justify-center bg-slate-800 border border-slate-700/80 rounded-xl text-slate-200 text-lg font-bold shadow-sm"
                  >
                    T{id}
                  </motion.div>
                ))
              ) : (
                <div className="text-slate-500 text-sm italic py-4 w-full text-center">Queue is empty.</div>
              )}
            </div>
          </div>

          {/* === Blocked Queue Zone === */}
          <div className="p-6 bg-slate-900/50 backdrop-blur-md border border-slate-800/50 rounded-2xl shadow-sm min-h-[13rem] flex flex-col">
            <h2 className="text-sm font-bold text-slate-400 mb-4 uppercase tracking-widest border-b border-slate-800/50 pb-3">Blocked (Mutex Wait)</h2>
            <div className="flex flex-wrap gap-3 py-2 flex-1 items-start content-start">
              {blockedQueue.length > 0 ? (
                blockedQueue.map((id) => (
                  <motion.div 
                    layout
                    initial={{ opacity: 0, scale: 0.8 }}
                    animate={{ opacity: 1, scale: 1 }}
                    whileHover={{ scale: 1.05 }}
                    key={`t-${id}`} 
                    className="w-14 h-14 flex items-center justify-center bg-rose-950/40 border border-rose-900/50 rounded-xl text-rose-300 text-lg font-bold shadow-sm"
                  >
                    T{id}
                  </motion.div>
                ))
              ) : (
                <div className="text-slate-500 text-sm italic py-4 w-full text-center">No blocked threads.</div>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* === Stdout Log Zone === */}
      <div className="p-5 bg-black/40 backdrop-blur-sm border border-slate-800/50 rounded-2xl overflow-hidden h-72 flex flex-col shadow-inner">
        <h2 className="text-xs font-bold uppercase tracking-widest text-slate-500 border-b border-slate-800/50 pb-3 mb-3 shrink-0">
          Terminal Stream
        </h2>
        <div className="space-y-1.5 text-[13px] text-emerald-400/90 flex-1 overflow-y-auto font-mono pb-2 pr-2">
          {terminalLogs.map((log, index) => (
            <div key={index} className="whitespace-pre">
              <span className="text-slate-600 mr-3">{'>'}</span>{log}
            </div>
          ))}
          {terminalLogs.length === 0 && <div className="text-slate-600 italic">Listening on ws://localhost:3001...</div>}
        </div>
      </div>
    </div>
  );
}
