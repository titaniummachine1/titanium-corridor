export function renderEvalBar(container, state) {
  const visible = state.settings.displayEvalBar;
  const margin = state.eval.margin ?? 0;
  const p1 = state.eval.p1 ?? 0.5;
  const scale = Math.max(0.02, Math.min(0.98, p1));
  const marginLabel = margin > 0 ? `+${margin}` : String(margin);

  const boardRow = container.closest('.board-row');
  boardRow?.classList.toggle('board-row--no-eval', !visible);

  container.className = 'eval-panel' + (visible ? ' eval-panel--visible' : '');
  container.innerHTML = `
    <div class="eval-bar ${state.settings.rotateBoard ? 'eval-bar--rotated' : ''}" title="White advantage: ${marginLabel} (B dist − W dist)">
      <div class="eval-bar__track"></div>
      <div class="eval-bar__fill" style="--scale: ${scale}"></div>
      <span class="eval-bar__label">${marginLabel}</span>
    </div>
  `;
}
