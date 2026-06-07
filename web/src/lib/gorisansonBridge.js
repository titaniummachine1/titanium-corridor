/** Bridge scraped UI actions ↔ gorisanson move tuples. */

export function actionToGorisansonMove(action) {
  const col = action.coordinate.column.charCodeAt(0) - 97;
  const row = action.coordinate.row - 1;
  if (action.wallType === 'h') {
    return [null, [row, col], null];
  }
  if (action.wallType === 'v') {
    return [null, null, [row, col]];
  }
  return [[row, col], null, null];
}

export function gorisansonMoveToAction(move) {
  const [pawn, horiz, vert] = move;
  if (pawn) {
    const [row, col] = pawn;
    return {
      coordinate: { column: String.fromCharCode(97 + col), row: row + 1 },
    };
  }
  if (horiz) {
    const [row, col] = horiz;
    return {
      coordinate: { column: String.fromCharCode(97 + col), row: row + 1 },
      wallType: 'h',
    };
  }
  const [row, col] = vert;
  return {
    coordinate: { column: String.fromCharCode(97 + col), row: row + 1 },
    wallType: 'v',
  };
}
