#!/usr/bin/env node
/**
 * Capture per-position search depth baselines via titanium genmove.
 *   node benchmark/capture_baseline.mjs
 *
 * Progress is saved after EACH position to benchmark/baseline_depths.json.
 * Re-run to resume — already-captured positions are skipped.
 *
 *   BASELINE_TIME_SEC=3 node benchmark/capture_baseline.mjs   # faster smoke run
 *   TITANIUM_BIN=engine/target/release/titanium.exe node ...
 */

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const OUT_PATH = path.join(ROOT, 'benchmark', 'baseline_depths.json');
const BIN = process.env.TITANIUM_BIN
  ?? path.join(ROOT, 'engine', 'target', 'release', process.platform === 'win32' ? 'titanium.exe' : 'titanium');
const TIME_SEC = Number(process.env.BASELINE_TIME_SEC ?? 10);
const FORCE = process.argv.includes('--force');

const POSITIONS = {
  opening_ply0: [],
  opening_ply4: ['e2', 'e8', 'e3', 'e7'],
  wall_heavy: [
    'e2', 'e8', 'e3', 'e7', 'e4', 'e6', 'd1h', 'e6h', 'd4', 'c6h', 'd5', 'a6h', 'e5', 'e5v',
  ],
  tq1_lost_ply69: [
    'e2', 'd2v', 'e4v', 'e2h', 'f2', 'f1v', 'e2', 'd1h', 'e8h', 'b1h', 'f2', 'a8h', 'f8v', 'd9',
    'f1', 'c9', 'e1', 'c8', 'd1', 'c8h', 'c1', 'f6v', 'b1', 'c7', 'a1', 'b7', 'b6h', 'f5h', 'a7v',
    'd4v', 'c6v', 'c7', 'c7h', 'b7', 'f3v', 'b8', 'a2', 'c8', 'b2', 'd8', 'c2', 'e8', 'd2', 'e7',
    'd3', 'e6', 'd4', 'e5', 'd5', 'e4', 'd6', 'e3', 'g2h', 'f3', 'e6', 'f4', 'e5', 'f5', 'e4', 'g5',
    'e3', 'g4', 'f3', 'h4', 'f4', 'h3', 'h3v', 'h4', 'f5',
  ],
  tq1_lost_ply73: [
    'e2', 'd2v', 'e4v', 'e2h', 'f2', 'f1v', 'e2', 'd1h', 'e8h', 'b1h', 'f2', 'a8h', 'f8v', 'd9',
    'f1', 'c9', 'e1', 'c8', 'd1', 'c8h', 'c1', 'f6v', 'b1', 'c7', 'a1', 'b7', 'b6h', 'f5h', 'a7v',
    'd4v', 'c6v', 'c7', 'c7h', 'b7', 'f3v', 'b8', 'a2', 'c8', 'b2', 'd8', 'c2', 'e8', 'd2', 'e7',
    'd3', 'e6', 'd4', 'e5', 'd5', 'e4', 'd6', 'e3', 'g2h', 'f3', 'e6', 'f4', 'e5', 'f5', 'e4', 'g5',
    'e3', 'g4', 'f3', 'h4', 'f4', 'h3', 'h3v', 'h4', 'f5', 'h5', 'g5', 'i5',
  ],
};

function log(msg) {
  const line = `[baseline] ${msg}\n`;
  process.stdout.write(line);
}

function loadExisting() {
  if (!fs.existsSync(OUT_PATH)) {
    return {
      capturedAt: new Date().toISOString(),
      timeSec: TIME_SEC,
      bin: BIN,
      positions: {},
    };
  }
  try {
    return JSON.parse(fs.readFileSync(OUT_PATH, 'utf8'));
  } catch {
    return {
      capturedAt: new Date().toISOString(),
      timeSec: TIME_SEC,
      bin: BIN,
      positions: {},
    };
  }
}

function save(out) {
  fs.writeFileSync(OUT_PATH, `${JSON.stringify(out, null, 2)}\n`);
}

function runSearch(name, moves) {
  return new Promise((resolve, reject) => {
    const args = ['genmove', '--engine', 'minimax', '--time', String(TIME_SEC * 1000), ...moves];
    const started = Date.now();
    const child = spawn(BIN, args, { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });

    let stderr = '';
    child.stderr.on('data', (d) => { stderr += d; });

    const heartbeat = setInterval(() => {
      const sec = ((Date.now() - started) / 1000).toFixed(0);
      log(`${name}: still searching… ${sec}s elapsed (budget ${TIME_SEC}s)`);
    }, 5000);

    child.on('error', (err) => {
      clearInterval(heartbeat);
      reject(err);
    });

    child.on('close', (code) => {
      clearInterval(heartbeat);
      const elapsedSec = ((Date.now() - started) / 1000).toFixed(1);
      if (code !== 0) {
        reject(new Error(`${name}: exit ${code} after ${elapsedSec}s\n${stderr.slice(-2000)}`));
        return;
      }
      const jsonLine = stderr.split(/\r?\n/).reverse().find((l) => l.startsWith('info json '));
      if (!jsonLine) {
        reject(new Error(`${name}: no info json after ${elapsedSec}s`));
        return;
      }
      const info = JSON.parse(jsonLine.slice('info json '.length));
      const last = info.depthLog?.at(-1);
      resolve({
        searchDepth: info.searchDepth,
        rootScore: info.rootScore,
        nodes: info.nodes,
        elapsedMs: info.elapsedMs,
        wallSec: Number(elapsedSec),
        lastDepth: last?.depth ?? info.searchDepth,
        lastScore: last?.score ?? info.rootScore,
        lastNodes: last?.nodes ?? info.nodes,
      });
    });
  });
}

async function main() {
  if (!fs.existsSync(BIN)) {
    log(`ERROR: binary not found: ${BIN}`);
    log('Run: cd engine && cargo build --release');
    process.exit(2);
  }

  const out = FORCE
    ? { capturedAt: new Date().toISOString(), timeSec: TIME_SEC, bin: BIN, positions: {} }
    : loadExisting();

  out.timeSec = TIME_SEC;
  out.bin = BIN;

  const names = Object.keys(POSITIONS);
  const todo = names.filter((n) => FORCE || !out.positions[n]);
  const done = names.length - todo.length;

  log(`binary: ${BIN}`);
  log(`budget: ${TIME_SEC}s per position · ${names.length} positions · ${done} already done · ${todo.length} to run`);
  log(`output: ${OUT_PATH} (saved after each position)`);
  log('tip: stop npm run dev first on Windows — it can lock titanium.exe');

  if (todo.length === 0) {
    log('all positions captured — use --force to re-run');
    console.log(JSON.stringify({ wrote: OUT_PATH, complete: true }));
    return;
  }

  const estSec = todo.length * (TIME_SEC + 2);
  log(`estimated wait: ~${estSec}s (${Math.ceil(estSec / 60)} min) if each search uses full budget`);

  for (const name of todo) {
    const moves = POSITIONS[name];
    log(`START ${name} (${moves.length} setup moves, ${TIME_SEC}s budget)`);
    const t0 = Date.now();
    out.positions[name] = await runSearch(name, moves);
    out.positions[name].capturedAt = new Date().toISOString();
    save(out);
    const sec = ((Date.now() - t0) / 1000).toFixed(1);
    log(
      `DONE  ${name}: d${out.positions[name].searchDepth} nodes=${out.positions[name].nodes} (${sec}s) → saved`
    );
  }

  out.capturedAt = new Date().toISOString();
  out.complete = true;
  save(out);
  log('finished');
  console.log(JSON.stringify({ wrote: OUT_PATH, positions: Object.keys(out.positions), complete: true }));
}

main().catch((e) => {
  log(`FAILED: ${e.message ?? e}`);
  process.exit(2);
});
