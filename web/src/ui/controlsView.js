export function renderControls(container, state, controller) {
  const { settings, aiThinking, timePresets, playerOptionGroups, timeBudgetHint, searchInfoLine } =
    state;

  container.innerHTML = `
    <section class="controls-card">
      <h1 class="app-title">Quoridor AI</h1>
      <p class="app-subtitle">Play · Human vs Ishtar, Ka, or local MCTS</p>

      <div class="control-group">
        <label class="control-label">Player 1 (moves first)</label>
        ${renderPlayerSelect('player1', settings.players[0], playerOptionGroups)}
      </div>

      <div class="control-group">
        <label class="control-label">Player 2</label>
        ${renderPlayerSelect('player2', settings.players[1], playerOptionGroups)}
      </div>

      <div class="control-group">
        <label class="control-label">AI Time</label>
        <select class="control-select" data-setting="time">
          ${timePresets
            .map(
              (preset) =>
                `<option value="${preset.id}" ${settings.timeToMove === preset.id ? 'selected' : ''}>${preset.label}</option>`,
            )
            .join('')}
        </select>
        <p class="time-hint">${escapeHtml(timeBudgetHint)}</p>
      </div>

      <div class="button-row">
        <button class="btn btn--primary" data-action="new-game">New Game</button>
        <button class="btn" data-action="undo" ${aiThinking ? 'disabled' : ''}>Undo</button>
      </div>

      <div class="toggle-group">
        <label class="toggle"><input type="checkbox" data-toggle="rotate" ${settings.rotateBoard ? 'checked' : ''} /> Rotate board</label>
        <label class="toggle"><input type="checkbox" data-toggle="coordinates" ${settings.displayCoordinates ? 'checked' : ''} /> Coordinates</label>
        <label class="toggle"><input type="checkbox" data-toggle="walls" ${settings.displayRemainingWalls ? 'checked' : ''} /> Wall count</label>
        <label class="toggle"><input type="checkbox" data-toggle="eval" ${settings.displayEvalBar ? 'checked' : ''} /> Eval bar</label>
      </div>

      <div class="status-panel">
        <div class="status-line"><span>Turn</span><strong>Player ${state.playerToMove}</strong></div>
        <div class="status-line"><span>Eval (P1)</span><strong>${Math.round((state.eval.p1 ?? 0.5) * 100)}%</strong></div>
        ${searchInfoLine ? `<div class="status-line status-line--muted"><span>AI</span><strong>${escapeHtml(searchInfoLine)}</strong></div>` : ''}
        ${
          state.eval.pv?.length
            ? `<div class="pv-line">PV: ${state.eval.pv.map((move) => (move.coordinate ? formatMove(move) : '?')).join(' ')}</div>`
            : ''
        }
      </div>
    </section>
  `;

  container.querySelector('[data-setting="player1"]')?.addEventListener('change', (event) => {
    controller.setPlayer(1, event.target.value);
  });
  container.querySelector('[data-setting="player2"]')?.addEventListener('change', (event) => {
    controller.setPlayer(2, event.target.value);
  });
  container.querySelector('[data-setting="time"]')?.addEventListener('change', (event) => {
    controller.setTimeToMove(Number(event.target.value));
  });

  container.querySelector('[data-action="new-game"]')?.addEventListener('click', () => {
    controller.newGame();
  });
  container.querySelector('[data-action="undo"]')?.addEventListener('click', () => {
    controller.undo();
  });

  container.querySelector('[data-toggle="rotate"]')?.addEventListener('change', () => {
    controller.toggleRotateBoard();
  });
  container.querySelector('[data-toggle="coordinates"]')?.addEventListener('change', () => {
    controller.toggleDisplayCoordinates();
  });
  container.querySelector('[data-toggle="walls"]')?.addEventListener('change', () => {
    controller.toggleDisplayRemainingWalls();
  });
  container.querySelector('[data-toggle="eval"]')?.addEventListener('change', () => {
    controller.toggleDisplayEvalBar();
  });
}

function renderPlayerSelect(name, value, groups) {
  const options = groups
    .map(
      (group) => `
      <optgroup label="${escapeHtml(group.label)}">
        ${group.options
          .map(
            (opt) =>
              `<option value="${opt.value}" ${opt.value === value ? 'selected' : ''} ${opt.disabled ? 'disabled' : ''}>${escapeHtml(opt.label)}</option>`,
          )
          .join('')}
      </optgroup>`,
    )
    .join('');

  return `<select class="control-select" data-setting="${name}">${options}</select>`;
}

function formatMove(action) {
  if (action.wallType) {
    const suffix = action.wallType === 'h' ? 'h' : 'v';
    return `${action.coordinate.column}${action.coordinate.row}${suffix}`;
  }
  return `${action.coordinate.column}${action.coordinate.row}`;
}

function escapeHtml(text) {
  return String(text)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;');
}
