/**
 * Gorisanson MCTS — play API for terminal benchmarks.
 */

import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const g = require('./load_gorisanson.cjs');

export const GORISANSON_UCT = 0.2;

/** Time preset → MCTS rollouts (matches gorisanson view.js levels). */
export const GORISANSON_TIME_SIMS = {
  intuition: 2_500,
  short: 7_500,
  medium: 20_000,
  long: 60_000,
};

export function createGorisansonGame() {
  return new g.Game(true);
}

export function cloneGorisansonGame(game) {
  return g.Game.clone(game);
}

export function chooseGorisansonMove(game, simulations = GORISANSON_TIME_SIMS.short) {
  const ai = new g.AI(simulations, GORISANSON_UCT, false, false);
  return ai.chooseNextMove(game);
}

export function applyGorisansonMove(game, move) {
  game.doMove(move, true);
}

export function winnerIndex(game) {
  return game.winner === null ? null : game.winner.index;
}
