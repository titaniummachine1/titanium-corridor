/**
 * Head-to-head game loop — gorisanson MCTS today, more engines later.
 */

import {
  applyGorisansonMove,
  chooseGorisansonMove,
  cloneGorisansonGame,
  createGorisansonGame,
  winnerIndex,
} from './gorisanson_ai.mjs';
import { moveLabel } from './gorisanson_moves.mjs';

const MAX_PLIES = 250;

/**
 * @param {{ id: string, simulations: number }} playerA
 * @param {{ id: string, simulations: number }} playerB
 */
export function playOneGame(playerA, playerB, { verbose = false } = {}) {
  let game = createGorisansonGame();
  let plies = 0;

  while (winnerIndex(game) === null && plies < MAX_PLIES) {
    const side = game.pawnOfTurn.index;
    const cfg = side === 0 ? playerA : playerB;
    const move = chooseGorisansonMove(game, cfg.simulations);
    if (verbose) {
      console.log(`  ply ${plies + 1} P${side + 1} (${cfg.id} ${cfg.simulations}): ${moveLabel(move)}`);
    }
    applyGorisansonMove(game, move);
    plies += 1;
  }

  const winner = winnerIndex(game);
  if (winner === null) {
    return { result: 'draw', winner: null, plies };
  }
  const winnerId = winner === 0 ? playerA.id : playerB.id;
  return { result: 'decided', winner: winnerId, winnerPawn: winner, plies };
}

export function playMatch(playerA, playerB, games, options = {}) {
  let scoreA = 0;
  let scoreB = 0;
  let draws = 0;
  const results = [];

  for (let i = 0; i < games; i++) {
    const swap = i % 2 === 1;
    const light = swap ? playerB : playerA;
    const dark = swap ? playerA : playerB;

    if (options.verbose) {
      console.log(`\nGame ${i + 1}/${games} — light=${light.id}(${light.simulations}) dark=${dark.id}(${dark.simulations})`);
    }

    const outcome = playOneGame(light, dark, options);
    results.push(outcome);

    if (outcome.result === 'draw') {
      draws += 1;
      scoreA += 0.5;
      scoreB += 0.5;
      continue;
    }

    if (outcome.winner === playerA.id) {
      scoreA += 1;
    } else if (outcome.winner === playerB.id) {
      scoreB += 1;
    }
  }

  return {
    playerA,
    playerB,
    games,
    scoreA,
    scoreB,
    draws,
    results,
  };
}

/** Simple Elo update from match score (0..games). */
export function eloFromMatch(scoreA, scoreB, games, ratingA = 1500, ratingB = 1500, k = 32) {
  const expectedA = 1 / (1 + 10 ** ((ratingB - ratingA) / 400));
  const actualA = scoreA / games;
  const newA = ratingA + k * (actualA - expectedA);
  const newB = ratingB + k * ((1 - actualA) - (1 - expectedA));
  return { ratingA: newA, ratingB: newB, expectedA };
}
