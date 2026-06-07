/**
 * Compare remote engine latency: minimal go vs our clone command sequence.
 * Usage: node benchmark/probe_intuition.mjs [ishtar|ka] [minimal|clone]
 */

const ENGINES = {
  ishtar: {
    uri: 'wss://quoridor-ai.com/ishtar-v3',
    visits: 2,
    parallelism: '1',
    static: [
      'setoption name display value false',
      'setoption name alternative_action_threshold value 0.1',
    ],
  },
  ka: {
    uri: 'wss://quoridor-ai.com/ka',
    visits: 1,
    parallelism: null,
    static: [],
  },
};

const engineKey = process.argv[2] ?? 'ishtar';
const mode = process.argv[3] ?? 'minimal';
const config = ENGINES[engineKey];

function elapsed(start) {
  return `${Date.now() - start}ms`;
}

function connectAndRun(commands) {
  const start = Date.now();
  const ws = new WebSocket(config.uri);

  ws.addEventListener('open', () => {
    console.log(`open ${elapsed(start)}`);
    ws.send(JSON.stringify({ token: 'rbt_token_*', version: '0.0.0' }));
    for (const command of commands) {
      console.log('>>', command);
      ws.send(command);
    }
  });

  ws.addEventListener('message', (event) => {
    const message = event.data.toString();
    if (message.startsWith('info time')) {
      console.log(`<< ${message.slice(0, 70)} … (${elapsed(start)})`);
    }
    if (message.startsWith('bestmove')) {
      console.log(`<< ${message} (${elapsed(start)})`);
      ws.close();
    }
  });

  setTimeout(() => {
    console.error('timeout', elapsed(start));
    process.exit(1);
  }, 30_000);
}

const minimal = [
  ...config.static,
  ...(config.parallelism ? [`setoption name parallelism value ${config.parallelism}`] : []),
  `setoption name visits value ${config.visits}`,
  'go',
];

const clone = [
  ...config.static,
  'makemove e2',
  'makemove e2',
  `setoption name visits value ${config.visits}`,
  ...(config.parallelism ? [`setoption name parallelism value ${config.parallelism}`] : []),
  'go',
];

connectAndRun(mode === 'clone' ? clone : minimal);
