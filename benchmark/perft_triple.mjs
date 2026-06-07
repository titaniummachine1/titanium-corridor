/**
 * Cross-oracle perft: scraped JS vs gorisanson vs Rust.
 * Default depth 3 = standard correctness gate (2_062_264 nodes).
 * Run: node benchmark/perft_triple.mjs [depth]
 */

import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

import {
  createGorisansonGame,
  gorisansonPerft,
  allLegalMoves,
  moveLabel,
} from './lib/gorisanson_moves.mjs';

const require = createRequire(import.meta.url);
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const { QuoridorBoard } = require(path.join(root, 'web/src/lib/gameLogic.js'));

const depth = Number(process.argv[2] ?? 3);

function scrapedPerft(d) {
  function clone(board) {
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
  function go(board, dd) {
    if (dd === 0) return 1n;
    let n = 0n;
    for (const a of board.validActions()) {
      const next = clone(board);
      next.takeAction(a);
      n += go(next, dd - 1);
    }
    return n;
  }
  return go(new QuoridorBoard(), d);
}

function rustPerft(d) {
  const out = execFileSync(
    'cargo',
    ['run', '--quiet', '--release', '--', 'perft', String(d)],
    { cwd: path.join(root, 'engine'), encoding: 'utf8' },
  );
  return BigInt(out.match(/perft \d+ (\d+)/)[1]);
}

const scraped = scrapedPerft(depth);
const gori = gorisansonPerft(createGorisansonGame(), depth);
const rust = rustPerft(depth);

const goriD1 = allLegalMoves(createGorisansonGame()).length;
const scrapedD1 = new QuoridorBoard().validActions().length;

console.log(`depth ${depth} startpos`);
console.log(`depth-1 move counts: scraped ${scrapedD1}  gorisanson ${goriD1}`);
console.log(`scraped JS:  ${scraped}`);
console.log(`gorisanson:  ${gori}`);
console.log(`Titanium:    ${rust}`);

const ok = scraped === gori && gori === rust;
console.log(ok ? '\nOK — all three match' : '\nMISMATCH — see perft_diff.mjs');
if (!ok) process.exit(1);
