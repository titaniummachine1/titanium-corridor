/**
 * Compact Unicode board for terminal benchmarks.
 * Inspired by Zoridor / quoridor.js — pawn grid + wall tally (full wall mesh is noisy in monospace).
 */

import { gorisansonMoveToAction } from './gorisanson_bridge.mjs';
import { toAlgebraic } from '../../web/src/lib/gameLogic.js';

export function moveAlgebraic(move) {
  return toAlgebraic(gorisansonMoveToAction(move));
}

/** Gorisanson row 0 = top (row 9 in standard notation). */
export function renderBoardAscii(game) {
  const grid = Array.from({ length: 9 }, () => Array(9).fill('·'));
  for (let i = 0; i < 2; i += 1) {
    const pawn = game.board.pawns[i];
    const mark = i === 0 ? 'W' : 'B';
    grid[pawn.position.row][pawn.position.col] = mark;
  }

  const lines = [];
  for (let row = 0; row < 9; row += 1) {
    const label = String(9 - row).padStart(2, ' ');
    lines.push(`${label} ${grid[row].join(' ')}`);
  }
  lines.push('    a b c d e f g h i');

  const wWalls = game.board.pawns[0].numberOfLeftWalls;
  const bWalls = game.board.pawns[1].numberOfLeftWalls;
  const hWalls = countPlacedWalls(game.board.walls.horizontal);
  const vWalls = countPlacedWalls(game.board.walls.vertical);
  lines.push(`    walls left W:${wWalls} B:${bWalls}   on board h:${hWalls} v:${vWalls}`);

  const placed = listPlacedWallsAlgebraic(game);
  if (placed.length > 0) {
    lines.push(`    placed: ${placed.join(' ')}`);
  }

  return lines.join('\n');
}

function listPlacedWallsAlgebraic(game) {
  const labels = [];
  const horizontal = game.board.walls.horizontal;
  const vertical = game.board.walls.vertical;
  for (let row = 0; row < 8; row += 1) {
    for (let col = 0; col < 8; col += 1) {
      if (horizontal[row][col]) {
        labels.push(moveAlgebraic([null, [row, col], null]));
      }
      if (vertical[row][col]) {
        labels.push(moveAlgebraic([null, null, [row, col]]));
      }
    }
  }
  return labels;
}

function countPlacedWalls(wallGrid) {
  let count = 0;
  for (let row = 0; row < 8; row += 1) {
    for (let col = 0; col < 8; col += 1) {
      if (wallGrid[row][col]) {
        count += 1;
      }
    }
  }
  return count;
}

export function sideName(index) {
  return index === 0 ? 'White' : 'Black';
}
