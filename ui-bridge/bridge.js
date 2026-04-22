import { Server } from 'socket.io';
import { createServer } from 'http';
import { spawn } from 'child_process';

const server = createServer();
const io = new Server(server, {
  cors: {
    origin: "*", // Allow all origins for development
    methods: ["GET", "POST"]
  }
});

io.on('connection', (socket) => {
  console.log(`Client connected: ${socket.id}`);
  
  socket.on('disconnect', () => {
    console.log(`Client disconnected: ${socket.id}`);
  });
});

const PORT = 3001;
server.listen(PORT, () => {
  console.log(`Bridge WebSocket server running on port ${PORT}`);
  console.log(`Waiting to spawn Rust process...`);
  
  // 1. Configure the Bridge Script: Spawning the Rust Process
  // We use child_process.spawn to run our binary.
  // cwd is set to '..' so it runs from the root of the Rust project.
  const rustProcess = spawn('cargo', ['run', '--example', 'showcase'], { cwd: '..' });

  // 2. Implement Output Parsing (Text-to-JSON Pipeline)
  rustProcess.stdout.on('data', (data) => {
    // Convert buffer to string and split by newlines
    const lines = data.toString().split('\n');

    lines.forEach((line) => {
      // Ignore empty lines to prevent spurious events
      if (line.trim().length === 0) return;

      // Create base event object
      const event = {
        type: 'LOG',
        text: line.trim(),
        threadId: null
      };

      // Extract the Thread ID using regex
      // Expecting formats like "[thread 1]" or "[main]"
      const threadMatch = line.match(/\[(?:thread )?(\d+|main)\]/);
      if (threadMatch && threadMatch[1]) {
        event.threadId = threadMatch[1];
      }

      // Determine UI state change via string/keyword matching
      if (line.includes('step') || line.includes('running')) {
        event.type = 'CPU_ACTIVE';
      } else if (line.includes('requesting lock') || line.includes('waiting for')) {
        event.type = 'BLOCKED';
      } else if (line.includes('LOCK ACQUIRED')) {
        event.type = 'MUTEX_ACQUIRED';
      } else if (line.includes('thread_exit') || line.includes('task complete') || line.includes('Done')) {
        event.type = 'EXITED';
      }

      // 3. Broadcasting
      // Emit the final parsed object to all connected WebSockets
      io.emit('os_event', event);

      // Log the emitted events to the Node terminal for debugging
      console.log('Emitted ->', event.type, event.threadId);
    });
  });

  // Also pipe stderr to help debug Rust compilation or runtime crashes
  rustProcess.stderr.on('data', (data) => {
    console.error(`[Rust STDERR]: ${data.toString()}`);
  });

  rustProcess.on('close', (code) => {
    console.log(`[Bridge] Rust child process exited with code ${code}`);
  });
});
