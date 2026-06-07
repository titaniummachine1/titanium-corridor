/**
 * Titanium greedy search via release CLI (`titanium genmove`).
 */

import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const BIN_NAME = process.platform === 'win32' ? 'titanium.exe' : 'titanium';
const DEFAULT_BIN = path.join(ROOT, 'engine', 'target', 'release', BIN_NAME);

function resolveBinary() {
  if (process.env.TITANIUM_BIN) {
    return process.env.TITANIUM_BIN;
  }
  return DEFAULT_BIN;
}

/**
 * @param {string[]} algebraicMoves — prior plies in UI notation (e2, d2h, …)
 * @returns {string} algebraic best move
 */
export function chooseTitaniumMove(algebraicMoves = []) {
  const bin = resolveBinary();
  const args = ['genmove', ...algebraicMoves];
  const result = spawnSync(bin, args, {
    encoding: 'utf8',
    cwd: ROOT,
    maxBuffer: 1024 * 1024,
  });

  if (result.error) {
    throw new Error(
      `Titanium binary not found at ${bin} — run: cargo build --release -p titanium (in engine/)`,
    );
  }
  if (result.status !== 0) {
    throw new Error(result.stderr?.trim() || `titanium genmove exited ${result.status}`);
  }

  const line = (result.stdout || '').trim().split(/\r?\n/).pop() || '';
  const match = /^bestmove\s+(\S+)/.exec(line);
  if (!match || match[1] === '(none)') {
    throw new Error(`no legal move from titanium: ${line}`);
  }
  return match[1];
}
