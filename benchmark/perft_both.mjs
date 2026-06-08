/**
 * Compare perft speed/results for Rust Titanium and Gorisanson.
 *
 * Defaults:
 * - Titanium depth 4
 * - Gorisanson depth 3
 *
 * Usage examples:
 *   node benchmark/perft_both.mjs
 *   node benchmark/perft_both.mjs --rust-depth 5 --gori-depth 3
 *   node benchmark/perft_both.mjs --moves "e2 e8 e3 e7"
 */

import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseAlgebraic, toAlgebraic } from '../web/src/lib/gameLogic.js';
import { actionToGorisansonMove, gorisansonMoveToAction } from './lib/gorisanson_bridge.mjs';
import { applyMove, createGorisansonGame, gorisansonPerft } from './lib/gorisanson_moves.mjs';
import { chooseTitaniumMove } from './lib/titanium_ai.mjs';
import {
    applyGorisansonMove as applyGorisansonSearchMove,
    chooseGorisansonMoveWithMeta,
    createGorisansonGame as createGorisansonSearchGame,
} from './lib/gorisanson_ai.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const BIN_NAME = process.platform === 'win32' ? 'titanium.exe' : 'titanium';
const DEFAULT_BIN = path.join(ROOT, 'engine', 'target', 'release', BIN_NAME);

function parseArgs(argv) {
    const opts = {
        rustDepth: 4,
        goriDepth: 3,
        searchTimeSec: 5,
        searchMaxSims: 2_000_000_000,
        openingMove: 'e2',
        moves: [],
        bin: process.env.TITANIUM_BIN || DEFAULT_BIN,
    };

    for (let i = 2; i < argv.length; i++) {
        const arg = argv[i];
        if (arg === '--rust-depth' && argv[i + 1]) {
            opts.rustDepth = Number(argv[++i]);
            continue;
        }
        if (arg === '--gori-depth' && argv[i + 1]) {
            opts.goriDepth = Number(argv[++i]);
            continue;
        }
        if (arg === '--moves' && argv[i + 1]) {
            opts.moves = String(argv[++i]).trim().split(/\s+/).filter(Boolean);
            continue;
        }
        if (arg === '--search-time' && argv[i + 1]) {
            opts.searchTimeSec = Number(argv[++i]);
            continue;
        }
        if (arg === '--search-max-sims' && argv[i + 1]) {
            opts.searchMaxSims = Number(argv[++i]);
            continue;
        }
        if (arg === '--opening-move' && argv[i + 1]) {
            opts.openingMove = String(argv[++i]);
            continue;
        }
        if (arg === '--bin' && argv[i + 1]) {
            opts.bin = String(argv[++i]);
            continue;
        }
    }

    if (!Number.isInteger(opts.rustDepth) || opts.rustDepth < 0) {
        throw new Error(`Invalid --rust-depth: ${opts.rustDepth}`);
    }
    if (!Number.isInteger(opts.goriDepth) || opts.goriDepth < 0) {
        throw new Error(`Invalid --gori-depth: ${opts.goriDepth}`);
    }
    if (!Number.isFinite(opts.searchTimeSec) || opts.searchTimeSec <= 0) {
        throw new Error(`Invalid --search-time: ${opts.searchTimeSec}`);
    }
    if (!Number.isFinite(opts.searchMaxSims) || opts.searchMaxSims <= 0) {
        throw new Error(`Invalid --search-max-sims: ${opts.searchMaxSims}`);
    }
    if (!opts.openingMove || typeof opts.openingMove !== 'string') {
        throw new Error('Invalid --opening-move');
    }

    return opts;
}

function fmtInt(value) {
    return Number(value).toLocaleString('en-US');
}

function fmtBigInt(value) {
    return value.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

async function runRustPerft(bin, depth, moves) {
    const args = ['perft', String(depth), ...moves];
    const started = performance.now();
    const result = await new Promise((resolve, reject) => {
        const child = spawn(bin, args, { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
        let stdout = '';
        let stderr = '';

        child.stdout.setEncoding('utf8');
        child.stderr.setEncoding('utf8');
        child.stdout.on('data', (chunk) => {
            stdout += chunk;
        });
        child.stderr.on('data', (chunk) => {
            stderr += chunk;
        });
        child.on('error', reject);
        child.on('close', (code) => {
            resolve({ code, stdout, stderr });
        });
    });

    if (result.code !== 0) {
        throw new Error(result.stderr.trim() || `Rust perft exited with code ${result.code}`);
    }

    const nodeMatch = /perft\s+\d+\s+(\d+)/i.exec(result.stdout);
    if (!nodeMatch) {
        throw new Error(`Could not parse nodes from Rust perft output:\n${result.stdout}`);
    }

    const elapsedMs = performance.now() - started;
    const nodes = BigInt(nodeMatch[1]);
    const nps = elapsedMs > 0 ? Number(nodes) / (elapsedMs / 1000) : 0;

    return {
        depth,
        nodes,
        elapsedMs,
        nps,
        raw: result.stdout.trim(),
    };
}

function runGorisansonPerft(depth, moves) {
    const game = createGorisansonGame();
    for (const algebraic of moves) {
        const action = parseAlgebraic(algebraic);
        const move = actionToGorisansonMove(action);
        applyMove(game, move);
    }

    const started = performance.now();
    const nodes = gorisansonPerft(game, depth);
    const elapsedMs = performance.now() - started;
    const nps = elapsedMs > 0 ? Number(nodes) / (elapsedMs / 1000) : 0;

    return {
        depth,
        nodes,
        elapsedMs,
        nps,
    };
}

function movesForSearch(opts) {
    if (opts.moves.length > 0) {
        return [...opts.moves];
    }
    return [opts.openingMove];
}

async function runSearchRolloutPerf(opts, moves) {
    const runAt = async (searchMoves) => {
        const rustResult = await chooseTitaniumMove(searchMoves, {
            timeSec: opts.searchTimeSec,
            maxSims: opts.searchMaxSims,
            engine: 'mcts',
        });

        const goriGame = createGorisansonSearchGame();
        for (const algebraic of searchMoves) {
            const action = parseAlgebraic(algebraic);
            applyGorisansonSearchMove(goriGame, actionToGorisansonMove(action));
        }
        const goriResult = chooseGorisansonMoveWithMeta(goriGame, {
            timeMs: Math.round(opts.searchTimeSec * 1000),
            maxSimulations: opts.searchMaxSims,
        });

        return {
            position: searchMoves,
            rust: {
                move: rustResult.move,
                simulations: rustResult.meta?.simulations ?? 0,
                nodes: rustResult.meta?.nodes ?? 0,
                elapsedMs: rustResult.meta?.elapsedMs ?? null,
                stoppedBy: rustResult.meta?.stoppedBy ?? null,
            },
            gorisanson: {
                move: toAlgebraic(gorisansonMoveToAction(goriResult.move)),
                simulations: goriResult.meta?.simulations ?? 0,
                elapsedMs: goriResult.meta?.elapsedMs ?? null,
                stoppedBy: goriResult.meta?.stoppedBy ?? null,
            },
        };
    };

    const firstPass = await runAt(moves);
    const openingHit =
        firstPass.rust.stoppedBy === 'opening' || firstPass.gorisanson.stoppedBy === 'opening';
    if (!openingHit) {
        return { ...firstPass, openingBypassed: false, openingProbe: null };
    }

    const forcedMove = firstPass.rust.move || firstPass.gorisanson.move;
    if (!forcedMove) {
        return { ...firstPass, openingBypassed: false, openingProbe: null };
    }

    const secondPass = await runAt([...moves, forcedMove]);
    return {
        ...secondPass,
        openingBypassed: true,
        openingProbe: firstPass,
    };
}

async function main() {
    const opts = parseArgs(process.argv);
    const searchMoves = movesForSearch(opts);

    console.log('Perft Compare: Rust Titanium vs Gorisanson');
    console.log(`Position: ${opts.moves.length ? opts.moves.join(' ') : '(initial)'}`);
    console.log(`Depths: rust=${opts.rustDepth} gorisanson=${opts.goriDepth}`);
    console.log(
        `Search rollout: time=${opts.searchTimeSec}s maxSims=${fmtInt(Math.round(opts.searchMaxSims))} position=${searchMoves.join(' ')}`,
    );
    console.log('');

    const rust = await runRustPerft(opts.bin, opts.rustDepth, opts.moves);
    const gori = runGorisansonPerft(opts.goriDepth, opts.moves);
    const rollout = await runSearchRolloutPerf(opts, searchMoves);

    console.log('Rust Titanium');
    console.log(`  nodes: ${fmtBigInt(rust.nodes)}`);
    console.log(`  time: ${rust.elapsedMs.toFixed(1)} ms`);
    console.log(`  nps:  ${fmtInt(Math.round(rust.nps))}`);
    console.log('');

    console.log('Gorisanson');
    console.log(`  nodes: ${fmtBigInt(gori.nodes)}`);
    console.log(`  time: ${gori.elapsedMs.toFixed(1)} ms`);
    console.log(`  nps:  ${fmtInt(Math.round(gori.nps))}`);
    console.log('');

    console.log('Search Rollout Perf');
    if (rollout.openingBypassed) {
        console.log('  note: opening-book reply detected; auto-advanced one ply to measure real rollout');
    }
    console.log(`  position: ${rollout.position.join(' ')}`);
    console.log('  Rust Titanium (MCTS)');
    console.log(`    bestmove: ${rollout.rust.move}`);
    console.log(`    simulations: ${fmtInt(rollout.rust.simulations)}`);
    console.log(
        `    nodes: ${rollout.rust.nodes > 0 ? fmtInt(rollout.rust.nodes) : 'n/a (not reported by engine in this mode)'}`,
    );
    console.log(
        `    elapsed: ${rollout.rust.elapsedMs == null ? 'n/a' : `${rollout.rust.elapsedMs.toFixed(1)} ms`}  stoppedBy: ${rollout.rust.stoppedBy ?? 'n/a'}`,
    );
    console.log('  Gorisanson (MCTS)');
    console.log(`    bestmove: ${rollout.gorisanson.move}`);
    console.log(`    simulations: ${fmtInt(rollout.gorisanson.simulations)}`);
    console.log('    nodes: n/a (not exposed by vendor API)');
    console.log(
        `    elapsed: ${rollout.gorisanson.elapsedMs == null ? 'n/a' : `${rollout.gorisanson.elapsedMs.toFixed(1)} ms`}  stoppedBy: ${rollout.gorisanson.stoppedBy ?? 'n/a'}`,
    );
}

main().catch((err) => {
    console.error(err?.stack || String(err));
    process.exit(1);
});
