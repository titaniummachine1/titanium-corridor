/**
 * Move codec between site algebraic notation and ACE v8 integer moves.
 * Matches engine/src/ace/mod.rs (algebraic_to_ace / ace_to_algebraic).
 */

export function algebraicToAceMove(algebraic) {
  const col = algebraic.charCodeAt(0) - 97;
  const row = Number.parseInt(algebraic[1], 10) - 1;
  if (algebraic.length === 2) {
    return (8 - row) * 9 + col;
  }
  const slot = (7 - row) * 8 + col;
  return algebraic.endsWith('h') ? 100 + slot : 200 + slot;
}

export function aceMoveToAlgebraic(move) {
  if (move < 100) {
    const r = (move / 9) | 0;
    const c = move % 9;
    return `${String.fromCharCode(97 + c)}${9 - r}`;
  }
  const base = move < 200 ? 100 : 200;
  const slot = move - base;
  const wr = (slot / 8) | 0;
  const wc = slot % 8;
  const suffix = move < 200 ? 'h' : 'v';
  return `${String.fromCharCode(97 + wc)}${8 - wr}${suffix}`;
}
