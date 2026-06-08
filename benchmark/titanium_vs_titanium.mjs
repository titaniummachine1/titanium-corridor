#!/usr/bin/env node
/**
 * Rust Titanium vs Rust Titanium (self-play) — 10s / 2B sim cap per move.
 */

import { eloFromMatch, playMatch } from './lib/match_engine.mjs';
import { RUST_TITANIUM_ID } from './lib/engine_ids.mjs';
import { BENCH_TIME_SEC, BENCH_MAX_SIMULATIONS, formatThinkBudget } from './lib/bench_limits.mjs';

async function main() {
    const games = Number(process.argv[2] ?? 4);
    const budget = { timeSec: BENCH_TIME_SEC, maxSimulations: BENCH_MAX_SIMULATIONS };
    const titanium = { id: RUST_TITANIUM_ID };

    console.log('Rust Titanium vs Rust Titanium (self-play)');
    console.log(`games=${games}  budget=${formatThinkBudget(budget)}`);

    const started = performance.now();
    const match = await playMatch(titanium, titanium, games, { ...budget, swapColors: true });
    const elapsed = (performance.now() - started) / 1000;

    console.log('');
    console.log(`Score: ${match.scoreA} - ${match.scoreB}  (draws ${match.draws})`);
    console.log(`Time:  ${elapsed.toFixed(1)}s`);
    console.log('(50/50 expected — same Rust genmove both sides)');

    process.exit(0);
}

main().catch((err) => {
    console.error(err?.stack || String(err));
    process.exit(1);
});
