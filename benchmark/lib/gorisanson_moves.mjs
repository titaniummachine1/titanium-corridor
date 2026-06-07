/**
 * Full legal move list from gorisanson/quoridor-ai Game (not MCTS probable subset).
 * Coords: 0-indexed pawn grid, walls 8x8 — matches Titanium internal indexing.
 */

import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const require = createRequire(import.meta.url);
const g = require('./load_gorisanson.cjs');

export function createGorisansonGame() {
  // false = AI (light pawn) moves first — standard Quoridor.
  return new g.Game(false);
}

export function cloneGorisansonGame(game) {
  return g.Game.clone(game);
}

export function moveLabel(move) {
  const [pawn, horiz, vert] = move;
  if (pawn) {
    const col = String.fromCharCode(97 + pawn[1]);
    return `${col}${pawn[0] + 1}`;
  }
  const [row, col] = horiz ?? vert;
  const c = String.fromCharCode(97 + col);
  const suffix = horiz ? 'h' : 'v';
  return `${c}${row + 1}${suffix}`;
}

/** All legal moves (pawn + walls), not MCTS probable-wall subset. */
export function allLegalMoves(game) {
  if (game.winner !== null) return [];

  const moves = [];
  const positions = game.validNextPositions;
  for (let r = 0; r < 9; r++) {
    for (let c = 0; c < 9; c++) {
      if (positions[r][c]) {
        moves.push([[r, c], null, null]);
      }
    }
  }

  if (game.pawnOfTurn.numberOfLeftWalls > 0) {
    const walls = game.validNextWalls;
    for (let r = 0; r < 8; r++) {
      for (let c = 0; c < 8; c++) {
        if (
          walls.horizontal[r][c] &&
          game.testIfExistPathsToGoalLinesAfterPlaceHorizontalWall(r, c)
        ) {
          moves.push([null, [r, c], null]);
        }
        if (
          walls.vertical[r][c] &&
          game.testIfExistPathsToGoalLinesAfterPlaceVerticalWall(r, c)
        ) {
          moves.push([null, null, [r, c]]);
        }
      }
    }
  }
  return moves;
}

export function applyMove(game, move) {
  game.doMove(move, true);
}

export function gorisansonPerft(game, depth) {
  if (depth === 0) return 1n;
  let nodes = 0n;
  for (const move of allLegalMoves(game)) {
    const next = cloneGorisansonGame(game);
    applyMove(next, move);
    nodes += gorisansonPerft(next, depth - 1);
  }
  return nodes;
}
