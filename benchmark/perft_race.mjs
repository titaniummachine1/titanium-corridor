/**
 * How deep can each engine perft in N seconds? (startpos)
 * Run: node benchmark/perft_race.mjs [seconds]
 */

import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { performance } from 'node:perf_hooks';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

import {
  createGorisansonGame,
  cloneGorisansonGame,
  gorisansonPerft,
} from './lib/gorisanson_moves.mjs';

const require = createRequire(import.meta.url);
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const { QuoridorBoard } = require(path.join(root, 'web/src/lib/gameLogic.js'));

const budgetSec = Number(process.argv[2] ?? 3);

function cloneScraped(board) {
  const copy = new QuoridorBoard();
  copy.playerToMove({ playerNum: board.playerToMove() });
  copy.moveNumber(board.moveNumber());
  for (let p = 1; p <= board.numPlayers(); p++) {
    copy.playerPosition({
      playerNum: p,
      coordinate: { ...board.playerPosition({ playerNum: p }) },
    });
    copy.wallsRemaining({
      playerNum: p,
      numWalls: board.wallsRemaining({ playerNum: p }),
    });
  }
  copy.setWalls(board.getWalls());
  return copy;
}

function scrapedPerft(board, d) {
  if (d === 0) return 1n;
  let nodes = 0n;
  for (const action of board.validActions()) {
    const next = cloneScraped(board);
    next.takeAction(action);
    nodes += scrapedPerft(next, d - 1);
  }
  return nodes;
}

function rustPerft(d) {
  const out = execFileSync(
    'cargo',
    ['run', '--quiet', '--release', '--', 'perft', String(d)],
    { cwd: path.join(root, 'engine'), encoding: 'utf8' },
  );
  const m = out.match(/perft \d+ (\d+)/);
  return m ? BigInt(m[1]) : null;
}

function maxDepthInBudget(name, runDepth) {
  let best = { depth: 0, nodes: 0n, ms: 0 };
  for (let d = 1; d <= 6; d++) {
    const t0 = performance.now();
    const nodes = runDepth(d);
    const ms = performance.now() - t0;
    if (ms > budgetSec * 1000) break;
    best = { depth: d, nodes, ms };
  }
  return { name, ...best };
}

console.log(`Perft race — ${budgetSec}s budget per engine (startpos)\n`);

const scraped = maxDepthInBudget('scraped UI (gameLogic.js)', (d) =>
  scrapedPerft(new QuoridorBoard(), d),
);
const gorisanson = maxDepthInBudget('gorisanson MCTS rules (full legal)', (d) =>
  gorisansonPerft(createGorisansonGame(), d),
);
const rust = maxDepthInBudget('Titanium Rust (release)', (d) => rustPerft(d));

for (const r of [scraped, gorisanson, rust]) {
  console.log(
    `${r.name.padEnd(36)} depth ${r.depth}  nodes ${String(r.nodes).padStart(8)}  ${(r.ms / 1000).toFixed(2)}s`,
  );
}

console.log('\nReference: pavlosdais C engine has no perft — move-gen only (_vendor/pavlosdais-quoridor).');
