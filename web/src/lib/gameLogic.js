/**
 * Scraped from quoridor-ai.netlify.app (index-xQ3tC4A2.js)
 * Client-side Quoridor rules engine — move generation, wall validation, notation.
 *
 * Original minified names restored:
 *   p9  → QuoridorBoard
 *   V   → Direction
 *   he  → WallType
 *   ly  → parseAlgebraic
 *   tu  → toAlgebraic
 *   pr  → formatCoordinate
 *   bt  → stepCoordinate
 *   vd  → transformCoordinate
 */

// ---------------------------------------------------------------------------
// Directions & wall types
// ---------------------------------------------------------------------------

const Direction = {
  Up: 'up',
  Down: 'down',
  Left: 'left',
  Right: 'right',
};

const WallType = {
  Vertical: 'v',
  Horizontal: 'h',
};

const COLUMN_BASE = 97; // 'a'

// ---------------------------------------------------------------------------
// Coordinate helpers
// ---------------------------------------------------------------------------

const allDirections = () => [
  Direction.Up,
  Direction.Right,
  Direction.Down,
  Direction.Left,
];

/** BFS expansion order per player (goal-seeking heuristic for path check). */
const pathCheckOrder = (playerNum) => {
  switch (playerNum) {
    case 1:
      return [Direction.Up, Direction.Right, Direction.Left, Direction.Down];
    case 2:
      return [Direction.Down, Direction.Right, Direction.Left, Direction.Up];
    case 3:
      return [Direction.Left, Direction.Up, Direction.Down, Direction.Right];
    case 4:
      return [Direction.Right, Direction.Up, Direction.Down, Direction.Left];
    default:
      return allDirections();
  }
};

/** Perpendicular directions used for jump resolution. */
const perpendicularDirections = (direction) => {
  switch (direction) {
    case Direction.Up:
    case Direction.Down:
      return [Direction.Left, Direction.Right];
    case Direction.Left:
    case Direction.Right:
      return [Direction.Up, Direction.Down];
    default:
      return [];
  }
};

function parseCoordinateText(text) {
  return { column: text[0], row: parseInt(text[1], 10) };
}

function formatCoordinate({ column, row }) {
  return `${column}${row}`;
}

function columnToIndex(column) {
  return column.charCodeAt(0) - COLUMN_BASE + 1;
}

function indexToColumn(index) {
  return String.fromCharCode(index + COLUMN_BASE - 1);
}

function stepCoordinate({ row, column }, direction) {
  switch (direction) {
    case Direction.Up:
      return { row: row + 1, column };
    case Direction.Down:
      return { row: row - 1, column };
    case Direction.Left:
      return { row, column: indexToColumn(columnToIndex(column) - 1) };
    case Direction.Right:
      return { row, column: indexToColumn(columnToIndex(column) + 1) };
    default:
      throw new Error(`Unknown direction: ${direction}`);
  }
}

function transformCoordinate(coordinate, directions) {
  return directions.reduce(
    (current, direction) => stepCoordinate(current, direction),
    coordinate,
  );
}

function coordinatesEqual(a, b) {
  return a.row === b.row && a.column === b.column;
}

// ---------------------------------------------------------------------------
// Algebraic notation (e2 = pawn, d2h = horizontal wall, d2v = vertical wall)
// ---------------------------------------------------------------------------

function parseAlgebraic(move) {
  const coordinate = parseCoordinateText(move.slice(0, 2));
  if (move.length > 2) {
    return {
      wallType: move[2] === 'h' ? WallType.Horizontal : WallType.Vertical,
      coordinate,
    };
  }
  return { coordinate };
}

function isWallAction(action) {
  return 'wallType' in action;
}

function isPawnAction(action) {
  return !('wallType' in action);
}

function toAlgebraic(action) {
  const base = formatCoordinate(action.coordinate);
  if (isWallAction(action)) {
    const suffix = action.wallType === WallType.Horizontal ? 'h' : 'v';
    return `${base}${suffix}`;
  }
  return base;
}

// ---------------------------------------------------------------------------
// QuoridorBoard — full rules engine (class p9)
// ---------------------------------------------------------------------------

class QuoridorBoard {
  constructor({ numCols = 9, numRows = 9, numPlayers = 2, wallsPerPlayer = 10 } = {}) {
    this._numCols = numCols;
    this._numRows = numRows;
    this._numPlayers = numPlayers;
    this._horizontalWalls = new Set();
    this._verticalWalls = new Set();
    this._wallsRemaining = Array(numPlayers).fill(wallsPerPlayer);
    this._playerToMove = 1;
    this._moveNumber = 1;
    this._playerPositions = this.initialPlayerPositions();
  }

  numColumns() {
    return this._numCols;
  }

  numRows() {
    return this._numRows;
  }

  numPlayers() {
    return this._numPlayers;
  }

  playerToMove({ playerNum } = {}) {
    if (playerNum !== undefined) {
      this._playerToMove = playerNum;
      return;
    }
    return this._playerToMove;
  }

  playerPosition({ playerNum, coordinate }) {
    if (coordinate !== undefined) {
      this._playerPositions[playerNum - 1] = coordinate;
      return;
    }
    return this._playerPositions[playerNum - 1];
  }

  moveNumber(value) {
    if (value !== undefined) {
      this._moveNumber = value;
      return;
    }
    return this._moveNumber;
  }

  wallsRemaining({ playerNum, numWalls }) {
    if (numWalls !== undefined) {
      this._wallsRemaining[playerNum - 1] = numWalls;
      return;
    }
    return this._wallsRemaining[playerNum - 1];
  }

  intoAction(action) {
    return typeof action === 'string' ? parseAlgebraic(action) : action;
  }

  takeAction(action) {
    const move = this.intoAction(action);

    if (isPawnAction(move)) {
      this._playerPositions[this._playerToMove - 1] = { ...move.coordinate };
    }

    if (isWallAction(move)) {
      const key = formatCoordinate(move.coordinate);
      const wallSet =
        move.wallType === WallType.Horizontal
          ? this._horizontalWalls
          : this._verticalWalls;
      wallSet.add(key);
      this._wallsRemaining[this._playerToMove - 1] -= 1;
    }

    this.updatePlayerToMove();
  }

  validActions() {
    return [...this.validPawnMoveActions(), ...this.validWallActions()];
  }

  validPawnMoveActions() {
    const moves = [];
    const from = this._playerPositions[this._playerToMove - 1];

    for (const direction of allDirections()) {
      if (!this.pawnCanMove(from, direction)) {
        continue;
      }

      const next = stepCoordinate(from, direction);

      if (!this.hasPawn(next)) {
        moves.push({ coordinate: next });
        continue;
      }

      if (this.pawnCanMove(next, direction)) {
        moves.push({ coordinate: stepCoordinate(next, direction) });
        continue;
      }

      for (const side of perpendicularDirections(direction)) {
        if (this.pawnCanMove(next, side)) {
          moves.push({ coordinate: stepCoordinate(next, side) });
        }
      }
    }

    return moves;
  }

  validWallActions() {
    return this.wallPlacements();
  }

  isValid(action) {
    const move = this.intoAction(action);
    if (isPawnAction(move)) {
      return this.isValidPawnMove(move);
    }
    return this.playerHasWalls() && this.isValidWallPlacement(move);
  }

  isValidPawnMove({ coordinate }) {
    return this.validPawnMoveActions().some((move) =>
      coordinatesEqual(move.coordinate, coordinate),
    );
  }

  isValidWallPlacement(wall) {
    return !this.collidesWithExistingWall(wall) && !this.isWallBlocking(wall);
  }

  playerHasWalls() {
    return this.wallsRemaining({ playerNum: this.playerToMove() }) > 0;
  }

  terminal() {
    for (let playerNum = 1; playerNum <= this._numPlayers; playerNum++) {
      if (this.isCoordinateGoal(playerNum, this.playerPosition({ playerNum }))) {
        return { isTerminal: true, playerNum };
      }
    }
    return { isTerminal: false, playerNum: 0 };
  }

  wallPlacements() {
    if (!this.playerHasWalls()) {
      return [];
    }

    const placements = [];

    for (const wallType of [WallType.Horizontal, WallType.Vertical]) {
      for (let row = 1; row < this._numRows; row++) {
        for (let col = 1; col < this._numCols; col++) {
          const coordinate = { column: indexToColumn(col), row };
          const wall = { coordinate, wallType };
          if (this.isValidWallPlacement(wall)) {
            placements.push(wall);
          }
        }
      }
    }

    return placements;
  }

  /** Would this wall block every player from their goal? */
  isWallBlocking({ coordinate, wallType }) {
    if (!this.canWallBlock({ coordinate, wallType })) {
      return false;
    }

    const wallSet =
      wallType === WallType.Horizontal
        ? this._horizontalWalls
        : this._verticalWalls;
    const key = formatCoordinate(coordinate);

    wallSet.add(key);

    let blocksSomeone = false;

    for (const [index, start] of this._playerPositions.entries()) {
      const playerNum = index + 1;
      const visited = new Set();
      const queue = [];

      const enqueueNeighbors = (position) => {
        for (const direction of pathCheckOrder(playerNum)) {
          queue.push([position, direction]);
        }
      };

      let canReachGoal = false;

      enqueueNeighbors(start);
      visited.add(formatCoordinate(start));

      while (queue.length > 0) {
        const [position, direction] = queue.pop();

        if (this.isCoordinateGoal(playerNum, position)) {
          canReachGoal = true;
          break;
        }

        const next = stepCoordinate(position, direction);
        const nextKey = formatCoordinate(next);

        if (visited.has(nextKey)) {
          continue;
        }

        if (!this.pawnCanMove(position, direction)) {
          continue;
        }

        visited.add(nextKey);
        enqueueNeighbors(next);
      }

      if (!canReachGoal) {
        blocksSomeone = true;
        break;
      }
    }

    wallSet.delete(key);
    return blocksSomeone;
  }

  pawnCanMove(from, direction) {
    const to = stepCoordinate(from, direction);
    if (!this.isInBounds(to)) {
      return false;
    }

    const blockingWallType =
      direction === Direction.Up || direction === Direction.Down
        ? WallType.Horizontal
        : WallType.Vertical;

    const side =
      direction === Direction.Up || direction === Direction.Down
        ? Direction.Left
        : Direction.Down;

    const wallAnchor =
      direction === Direction.Up || direction === Direction.Right ? from : to;
    const sideAnchor = stepCoordinate(wallAnchor, side);

    return !(
      this.hasWall(wallAnchor, blockingWallType) ||
      this.hasWall(sideAnchor, blockingWallType)
    );
  }

  isInBounds({ column, row }) {
    const col = columnToIndex(column);
    return row >= 1 && row <= this._numRows && col >= 1 && col <= this._numCols;
  }

  hasWall(coordinate, wallType) {
    const set =
      wallType === WallType.Horizontal
        ? this._horizontalWalls
        : this._verticalWalls;
    return set.has(formatCoordinate(coordinate));
  }

  hasPawn({ column, row }) {
    return this._playerPositions.some(
      (pawn) => pawn.column === column && pawn.row === row,
    );
  }

  initialPlayerPositions() {
    const centerRow = Math.floor((this._numRows + 1) / 2);
    const centerCol = indexToColumn(Math.floor((this._numCols + 1) / 2));
    const positions = [];

    positions[0] = { column: centerCol, row: 1 };

    if (this._numPlayers >= 2) {
      positions[1] = { column: centerCol, row: this._numRows };
    }
    if (this._numPlayers >= 3) {
      positions[2] = { column: 'a', row: centerRow };
    }
    if (this._numPlayers >= 4) {
      positions[3] = { column: indexToColumn(this._numCols), row: centerRow };
    }

    return positions;
  }

  isCoordinateGoal(playerNum, { row, column }) {
    switch (playerNum) {
      case 1:
        return row === this._numRows;
      case 2:
        return row === 1;
      case 3:
        return column === indexToColumn(this._numCols);
      case 4:
        return column === 'a';
      default:
        return false;
    }
  }

  updatePlayerToMove() {
    this._playerToMove = (this._playerToMove % this._numPlayers) + 1;
    if (this._playerToMove === 1) {
      this._moveNumber += 1;
    }
  }

  collidesWithExistingWall({ coordinate, wallType }) {
    const isHorizontal = wallType === WallType.Horizontal;
    const perpendicular = isHorizontal ? WallType.Vertical : WallType.Horizontal;
    const leftOrUp = isHorizontal ? [Direction.Left] : [Direction.Up];
    const rightOrDown = isHorizontal ? [Direction.Right] : [Direction.Down];
    const noOffset = [];

    const candidates = [
      { offsets: noOffset, wallType },
      { offsets: noOffset, wallType: perpendicular },
      { offsets: leftOrUp, wallType },
      { offsets: rightOrDown, wallType },
    ];

    return this.someWallAtOffsets(coordinate, candidates);
  }

  someWallAtOffsets(coordinate, candidates) {
    return candidates.some((candidate) =>
      this.wallAtOffset(coordinate, candidate),
    );
  }

  wallAtOffset(coordinate, { offsets, wallType }) {
    return this.hasWall(transformCoordinate(coordinate, offsets), wallType);
  }

  canWallBlock({ coordinate, wallType }) {
    const { sideACandidates, sideBCandidates, middleCandidates } =
      this.touchingWallCandidates(wallType);
    const [onSideAEdge, onSideBEdge] = this.sideOnEdge({ coordinate, wallType });

    const sideA = onSideAEdge || this.someWallAtOffsets(coordinate, sideACandidates);
    const sideB = onSideBEdge || this.someWallAtOffsets(coordinate, sideBCandidates);
    const middle = this.someWallAtOffsets(coordinate, middleCandidates);

    return (sideA && sideB) || (sideA && middle) || (sideB && middle);
  }

  sideOnEdge({ coordinate, wallType }) {
    const isHorizontal = wallType === WallType.Horizontal;
    const col = columnToIndex(coordinate.column);
    const row = coordinate.row;

    const onA =
      (isHorizontal && col === 1) || (!isHorizontal && row === this._numRows - 1);
    const onB =
      (isHorizontal && col === this._numCols) || (!isHorizontal && row === 1);

    return [onA, onB];
  }

  touchingWallCandidates(wallType) {
    const isHorizontal = wallType === WallType.Horizontal;
    const perpendicular = isHorizontal ? WallType.Vertical : WallType.Horizontal;
    const sideA = isHorizontal ? Direction.Left : Direction.Up;
    const sideB = isHorizontal ? Direction.Right : Direction.Down;
    const innerA = isHorizontal ? Direction.Up : Direction.Left;
    const innerB = isHorizontal ? Direction.Down : Direction.Right;

    const sideCandidates = (offset) => [
      { offsets: [offset], wallType: perpendicular },
      { offsets: [innerA, offset], wallType: perpendicular },
      { offsets: [innerB, offset], wallType: perpendicular },
      { offsets: [offset, offset], wallType },
    ];

    const middleCandidates = () => [
      { offsets: [innerA], wallType: perpendicular },
      { offsets: [innerB], wallType: perpendicular },
    ];

    return {
      sideACandidates: sideCandidates(sideA),
      sideBCandidates: sideCandidates(sideB),
      middleCandidates: middleCandidates(),
    };
  }

  getWalls() {
    const horizontal = [...this._horizontalWalls].map((key) => [
      WallType.Horizontal,
      parseCoordinateText(key),
    ]);
    const vertical = [...this._verticalWalls].map((key) => [
      WallType.Vertical,
      parseCoordinateText(key),
    ]);
    return [...horizontal, ...vertical];
  }

  setWalls(walls) {
    this._horizontalWalls.clear();
    this._verticalWalls.clear();

    for (const [wallType, coordinate] of walls) {
      const key = formatCoordinate(coordinate);
      if (wallType === WallType.Horizontal) {
        this._horizontalWalls.add(key);
      } else {
        this._verticalWalls.add(key);
      }
    }
  }

  walls(wallList) {
    if (wallList !== undefined) {
      this.setWalls(wallList);
      return;
    }
    return this.getWalls();
  }
}

/** Rebuild a QuoridorBoard from Redux game state snapshot. */
function boardFromGameState(gameSlice) {
  const { numCols, numRows, numPlayers } = gameSlice;
  const { playerToMove, moveNumber, wallsRemaining, playerPositions, wallsByPlayer } =
    gameSlice.currentState;

  const board = new QuoridorBoard({ numCols, numRows, numPlayers });
  board.playerToMove({ playerNum: playerToMove });
  board.moveNumber(moveNumber);

  wallsRemaining.forEach((count, index) => {
    board.wallsRemaining({ playerNum: index + 1, numWalls: count });
  });

  playerPositions.forEach((coordinate, index) => {
    board.playerPosition({ playerNum: index + 1, coordinate });
  });

  board.walls(wallsByPlayer.map(([, coordinate, wallType]) => [wallType, coordinate]));
  return board;
}

export {
  Direction,
  WallType,
  QuoridorBoard,
  parseAlgebraic,
  toAlgebraic,
  formatCoordinate,
  transformCoordinate,
  boardFromGameState,
  isWallAction,
  isPawnAction,
};
