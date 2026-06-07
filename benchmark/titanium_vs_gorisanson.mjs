#!/usr/bin/env node
/**
 * Titanium (greedy Rust) vs Gorisanson MCTS — terminal benchmark.
 *
 *   node benchmark/titanium_vs_gorisanson.mjs
 *   node benchmark/titanium_vs_gorisanson.mjs --games 10 --gorisanson 7500
 */

import { eloFromMatch, playMatch } from './lib/match_engine.mjs';
import { GORISANSON_TIME_SIMS } from './lib/gorisanson_ai.mjs';

function parseArgs(argv) {
  const opts = {
    games: 4,
    gorisanson: GORISANSON_TIME_SIMS.short,
    verbose: false,
  };

  for (let i = 2; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--games' && argv[i + 1]) {
      opts.games = Number(argv[++i]);
    } else if (arg === '--gorisanson' && argv[i + 1]) {
      opts.gorisanson = Number(argv[++i]);
    } else if (arg === '--verbose' || arg === '-v') {
      opts.verbose = true;
    }
  }

  return opts;
}

const opts = parseArgs(process.argv);
const log = console.log.bind(console);

if (!opts.verbose) {
  console.log = () => {};
}

const titanium = { id: 'titanium' };
const gorisanson = { id: 'gorisanson', simulations: opts.gorisanson };

log('Titanium (greedy) vs Gorisanson MCTS');
log(`games=${opts.games}  gorisanson=${opts.gorisanson} sims`);

const started = performance.now();
const match = playMatch(titanium, gorisanson, opts.games, { verbose: opts.verbose });
const elapsed = (performance.now() - started) / 1000;

const { ratingA, ratingB, expectedA } = eloFromMatch(
  match.scoreA,
  match.scoreB,
  opts.games,
  1400,
  1600,
);

log('');
log(`Score: titanium ${match.scoreA} — gorisanson ${match.scoreB}  (draws ${match.draws})`);
log(`Time:  ${elapsed.toFixed(1)}s  (${(elapsed / opts.games).toFixed(1)}s/game)`);
log('');
log('Elo (provisional):');
log(`  titanium   → ${ratingA.toFixed(0)}`);
log(`  gorisanson → ${ratingB.toFixed(0)}`);
log(`  expected titanium score: ${(expectedA * 100).toFixed(1)}%`);

if (match.scoreB > match.scoreA) {
  log(`\nGorisanson leads as expected at ${opts.gorisanson} sims`);
} else if (match.scoreA > match.scoreB) {
  log('\nTitanium greedy won — MCTS may need more sims or variance is high');
} else {
  log('\nTied — try --games 20');
}

process.exit(0);
