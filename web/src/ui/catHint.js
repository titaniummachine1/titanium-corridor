export function renderCatHint(container, state, controller) {
  const existing = container.querySelector('.cat-hint');
  if (!state.showCatHint) {
    existing?.remove();
    return;
  }

  if (existing) {
    return;
  }

  const hint = document.createElement('div');
  hint.className = 'cat-hint';
  hint.innerHTML = `
    <div class="cat-hint__card">
      <p class="cat-hint__title">CAT vision</p>
      <div class="cat-hint__bar" aria-hidden="true"></div>
      <p class="cat-hint__labels"><span>cold</span><span>warm</span><span>hot</span></p>
      <p class="cat-hint__text">Numbers = raw engine heat in cm (exactly what search sees). Tint anchors: ≥60 warm corridor, ≥160 hot (tactical / no LMR). Cold fringe shows a faint number, no tint. Dark squares are unreachable. Wall outlines = searchable walls.</p>
      <button type="button" class="btn btn--primary btn--small" data-action="dismiss-cat-hint">Got it</button>
    </div>
  `;
  hint.querySelector('[data-action="dismiss-cat-hint"]')?.addEventListener('click', () => {
    controller.dismissCatHint();
  });
  container.appendChild(hint);
}
