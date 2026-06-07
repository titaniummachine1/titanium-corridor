export function renderEvalBar(container, state) {
  const visible = state.settings.displayEvalBar;
  const p1 = state.eval.p1 ?? 0.5;
  const scale = Math.max(0.02, Math.min(0.98, p1));

  container.className = 'eval-panel' + (visible ? ' eval-panel--visible' : '');
  container.innerHTML = `
    <div class="eval-bar ${state.settings.rotateBoard ? 'eval-bar--rotated' : ''}">
      <div class="eval-bar__track"></div>
      <div class="eval-bar__fill" style="--scale: ${scale}"></div>
    </div>
  `;
}
