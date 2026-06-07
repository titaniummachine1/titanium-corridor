/** UI labels — player 1 = White (moves first), player 2 = Black. */

export function playerColorName(playerNum) {
  return playerNum === 1 ? 'White' : 'Black';
}

export function playerColorLabel(playerNum) {
  if (playerNum === 1) {
    return 'White (moves first)';
  }
  return 'Black';
}
