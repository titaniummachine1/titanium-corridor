import { WallType, formatCoordinate, toAlgebraic } from '../lib/gameLogic.js';

// Re-export column helper used by board grid
function indexToColumnLocal(index) {
  return String.fromCharCode(index + 96);
}

function columnLabel(colIndex, numCols) {
  return indexToColumnLocal(colIndex);
}

function rowLabel(rowIndex, numRows) {
  return String(numRows - rowIndex);
}

export function renderBoard(container, state, controller) {
  const {
    board,
    validActions,
    playerPositions,
    wallsRemaining,
    winner,
    playerToMove,
    settings,
    engineStatus,
    aiThinking,
  } = state;

  const numRows = board.numRows();
  const numCols = board.numColumns();
  const validKeys = new Set(validActions.map((action) => toAlgebraic(action)));

  const wallOwners = new Map();
  for (const [playerNum, coordinate, wallType] of state.wallsByPlayer) {
    wallOwners.set(toAlgebraic({ coordinate, wallType }), playerNum);
  }

  const lastKey = state.lastAction ? toAlgebraic(state.lastAction) : null;

  container.innerHTML = '';
  container.className = 'board-panel' + (settings.rotateBoard ? ' board-panel--rotated' : '');

  const boardShell = document.createElement('div');
  boardShell.className = 'board';

  const topStatus = document.createElement('div');
  topStatus.className = 'engine-state engine-state--p2';
  topStatus.appendChild(renderTurnIndicator(2, playerToMove, settings.players[1], engineStatus, aiThinking));

  const bottomStatus = document.createElement('div');
  bottomStatus.className = 'engine-state engine-state--p1';
  bottomStatus.appendChild(renderTurnIndicator(1, playerToMove, settings.players[0], engineStatus, aiThinking));

  const rowLabels = document.createElement('div');
  rowLabels.className = 'coord-row' + (settings.displayCoordinates ? ' coord-row--visible' : '');
  for (let row = 0; row < numRows; row++) {
    const label = document.createElement('span');
    label.className = 'coord-label';
    label.textContent = rowLabel(row, numRows);
    rowLabels.appendChild(label);
    if (row < numRows - 1) {
      const spacer = document.createElement('span');
      spacer.className = 'coord-spacer';
      rowLabels.appendChild(spacer);
    }
  }

  const wallsP2 = document.createElement('div');
  wallsP2.className = 'wall-rack wall-rack--p2';
  wallsP2.appendChild(renderWallRack(2, wallsRemaining[1], settings, controller));

  const wallsP1 = document.createElement('div');
  wallsP1.className = 'wall-rack wall-rack--p1';
  wallsP1.appendChild(renderWallRack(1, wallsRemaining[0], settings, controller));

  const grid = document.createElement('div');
  grid.className = 'board-grid';
  grid.style.gridTemplateColumns = `repeat(${numCols * 2 - 1}, 1fr)`;
  grid.style.gridTemplateRows = `repeat(${numRows * 2 - 1}, 1fr)`;

  for (let p = 0; p < numRows * 2 - 1; p++) {
    for (let h = 0; h < numCols * 2 - 1; h++) {
      const row = numRows - Math.floor(p / 2);
      const col = Math.floor(h / 2) + 1;
      const isEvenRow = p % 2 === 0;
      const isEvenCol = h % 2 === 0;

      let cellType;
      if (isEvenRow && isEvenCol) {
        cellType = 'square';
      } else if (isEvenRow) {
        cellType = 'verticalWall';
      } else if (isEvenCol) {
        cellType = 'horizontalWall';
      } else {
        cellType = 'wallIntersection';
      }

      const cell = document.createElement('div');
      cell.className = `cell cell--${cellType}`;
      cell.dataset.cellType = cellType;

      if (cellType === 'square') {
        const coordinate = { row, column: indexToColumnLocal(col) };
        const key = formatCoordinate(coordinate);
        const pawnPlayer = playerPositions.findIndex(
          (pos) => pos.row === coordinate.row && pos.column === coordinate.column,
        );
        const isValid = validKeys.has(key);
        const isHumanTurn = controller.session.isHumanTurn(settings.players);
        const isPrev = lastKey === key;

        cell.classList.toggle('cell--valid', isValid && isHumanTurn && pawnPlayer < 0);
        cell.classList.toggle('cell--prev', isPrev);

        if (pawnPlayer >= 0) {
          const pawn = document.createElement('div');
          pawn.className = `pawn pawn--player${pawnPlayer + 1}`;
          cell.appendChild(pawn);
        }

        if (isValid && isHumanTurn) {
          cell.dataset.action = key;
        }
      }

      if (cellType === 'horizontalWall' || cellType === 'verticalWall') {
        const coordinate = {
          row: row - 1,
          column: indexToColumnLocal(col),
        };
        const wallType = cellType === 'horizontalWall' ? WallType.Horizontal : WallType.Vertical;
        const key = toAlgebraic({ coordinate, wallType });
        const owner = wallOwners.get(key);
        const isValid = validKeys.has(key);
        const isHumanTurn = controller.session.isHumanTurn(settings.players);
        const isPrev = lastKey === key;

        if (owner) {
          const wall = document.createElement('div');
          wall.className = `wall wall--placed wall--${wallType === WallType.Horizontal ? 'h' : 'v'} wall--player${owner}`;
          cell.appendChild(wall);
        } else if (isValid && isHumanTurn) {
          cell.classList.add('cell--valid');
          cell.dataset.action = key;
          const ghost = document.createElement('div');
          ghost.className = `wall wall--ghost wall--${wallType === WallType.Horizontal ? 'h' : 'v'}`;
          cell.appendChild(ghost);
        }

        cell.classList.toggle('cell--prev', isPrev);
      }

      grid.appendChild(cell);
    }
  }

  const wallColumn = document.createElement('div');
  wallColumn.className = 'wall-rack';
  wallColumn.append(wallsP2, wallsP1);

  boardShell.append(topStatus, rowLabels, grid, wallColumn, bottomStatus);
  container.appendChild(boardShell);

  if (winner) {
    const banner = document.createElement('div');
    banner.className = 'winner-banner';
    banner.textContent = `Player ${winner} wins!`;
    container.appendChild(banner);
  }

  grid.addEventListener('click', (event) => {
    const target = event.target.closest('[data-action]');
    if (!target) {
      return;
    }
    const actionKey = target.dataset.action;
    if (!actionKey) {
      return;
    }

    if (actionKey.length === 2) {
      controller.tryAction({ coordinate: parseCoord(actionKey) });
      return;
    }

    const wallType = actionKey[2] === 'h' ? WallType.Horizontal : WallType.Vertical;
    controller.tryAction({
      coordinate: parseCoord(actionKey.slice(0, 2)),
      wallType,
    });
  });
}

function parseCoord(text) {
  return { column: text[0], row: parseInt(text[1], 10) };
}

function renderWallRack(playerNum, remaining, settings, controller) {
  const rack = document.createElement('div');
  rack.className = 'wall-rack-inner';

  const count = document.createElement('div');
  count.className = 'walls-count' + (settings.displayRemainingWalls ? ' walls-count--visible' : '');
  count.textContent = String(remaining);
  rack.appendChild(count);

  for (let index = 0; index < 10; index++) {
    const slot = document.createElement('div');
    slot.className = 'wall-slot' + (index < remaining ? ' wall-slot--available' : '');
    slot.classList.add(`wall-slot--player${playerNum}`);
    rack.appendChild(slot);
  }

  rack.addEventListener('click', () => controller.toggleDisplayRemainingWalls?.());
  return rack;
}

function renderTurnIndicator(playerNum, playerToMove, playerType, engineStatus, aiThinking) {
  const wrap = document.createElement('div');
  wrap.className = 'turn-indicator';

  if (playerToMove !== playerNum) {
    return wrap;
  }

  if (playerType === 'human') {
    const dot = document.createElement('div');
    dot.className = `turn-dot turn-dot--player${playerNum}`;
    dot.title = 'Your turn';
    wrap.appendChild(dot);
    return wrap;
  }

  const status = engineStatus[playerType] ?? 'idle';
  const spinner = document.createElement('div');
  spinner.className = 'engine-spinner';
  spinner.title =
    status === 'error'
      ? 'Engine connection error'
      : aiThinking
        ? 'Engine is thinking...'
        : 'Connecting...';
  if (status === 'error') {
    spinner.classList.add('engine-spinner--error');
    spinner.textContent = '!';
  }
  wrap.appendChild(spinner);
  return wrap;
}
