/**
 * Move codec between our algebraic notation and Quoridor v3 integer moves.
 *
 * v3 encoding (from quoridor.html):
 *   pawn: 0–80 (cell index, row 0 = top)
 *   horizontal wall: 100 + slot (slot 0–63 on 8×8 wall grid)
 *   vertical wall: 200 + slot
 */

const BOARD_ROWS = 9;
const WALL_ROWS = 8;

export function algebraicToV3Move(algebraic) {
  const col = algebraic.charCodeAt(0) - 97;
  const row = Number.parseInt(algebraic.slice(1), 10);
  if (algebraic.length === 2) {
    const v3Row = BOARD_ROWS - row;
    return v3Row * 9 + col;
  }
  const wallRow = WALL_ROWS - row;
  const slot = wallRow * 8 + col;
  return algebraic.endsWith('h') ? 100 + slot : 200 + slot;
}

export function v3MoveToAlgebraic(move) {
  if (move < 100) {
    const v3Row = (move / 9) | 0;
    const col = move % 9;
    const row = BOARD_ROWS - v3Row;
    return `${String.fromCharCode(97 + col)}${row}`;
  }
  const slot = move % 100;
  const wallRow = (slot / 8) | 0;
  const col = slot % 8;
  const row = WALL_ROWS - wallRow;
  const suffix = move < 200 ? 'h' : 'v';
  return `${String.fromCharCode(97 + col)}${row}${suffix}`;
}

export function algebraicMovesToV3(algebraicMoves) {
  return algebraicMoves.map(algebraicToV3Move);
}
