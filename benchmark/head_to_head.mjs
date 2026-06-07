#!/usr/bin/env node
/**
 * Terminal head-to-head — no browser, no user input.
 *
 *   node benchmark/head_to_head.mjs
 *   node benchmark/head_to_head.mjs --games 10 --p1 7500 --p2 20000
 */

import { eloFromMatch, playMatch } from './lib/match_engine.mjs';
import { GORISANSON_TIME_SIMS } from './lib/gorisanson_ai.mjs';

function parseArgs(argv) {
  const opts = {
    games: 4,
    p1: GORISANSON_TIME_SIMS.short,
    p2: GORISANSON_TIME_SIMS.medium,
    verbose: false,
  };

  for (let i = 2; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--games' && argv[i + 1]) {
      opts.games = Number(argv[++i]);
    } else if (arg === '--p1' && argv[i + 1]) {
      opts.p1 = Number(argv[++i]);
    } else if (arg === '--p2' && argv[i + 1]) {
      opts.p2 = Number(argv[++i]);
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

const playerA = { id: 'gorisanson', simulations: opts.p1 };
const playerB = { id: 'gorisanson', simulations: opts.p2 };

log('Titanium head-to-head (gorisanson MCTS)');
log(`games=${opts.games}  P1=${playerA.simulations} sims  P2=${playerB.simulations} sims`);

const started = performance.now();
const match = playMatch(playerA, playerB, opts.games, { verbose: opts.verbose });
const elapsed = (performance.now() - started) / 1000;

const { ratingA, ratingB, expectedA } = eloFromMatch(
  match.scoreA,
  match.scoreB,
  opts.games,
);

log('');
log(`Score: ${match.scoreA} - ${match.scoreB}  (draws ${match.draws})`);
log(`Time:  ${elapsed.toFixed(1)}s  (${(elapsed / opts.games).toFixed(1)}s/game)`);
log('');
log('Elo (provisional, from this match only):');
log(`  ${playerA.simulations} sims → ${ratingA.toFixed(0)}`);
log(`  ${playerB.simulations} sims → ${ratingB.toFixed(0)}`);
log(`  expected score for weaker side: ${(expectedA * 100).toFixed(1)}%`);

if (match.scoreB > match.scoreA) {
  log(`\nOK — stronger preset (${opts.p2}) leads (${match.scoreB}/${opts.games})`);
} else if (match.scoreA > match.scoreB) {
  log(`\nNote — lower sim count won; variance or opening luck (try --games 20)`);
} else {
  log('\nTied — try more games');
}

process.exit(0);
