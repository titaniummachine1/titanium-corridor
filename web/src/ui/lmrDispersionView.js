/** Ranked LMR effort dispersion — node %, child depth, ply cut per root move. */

function escapeHtml(text) {
  return String(text)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;');
}

function fmtShare(pct) {
  const n = Number(pct);
  if (!Number.isFinite(n) || n <= 0) {
    return '0%';
  }
  if (n < 1) {
    return '<1%';
  }
  if (n >= 10) {
    return `${Math.round(n)}%`;
  }
  return `${n.toFixed(1)}%`;
}

function rowTitle(row) {
  const parts = [
    `#${row.order + 1}`,
    row.searched ? `${row.nodes.toLocaleString()} nodes` : 'plan only',
    row.childDepthUsed > 0 ? `child d${row.childDepthUsed}` : '',
    row.reduction > 0 ? `cut −${row.reduction}` : 'full depth',
    row.catCm > 0 ? `CAT ${row.catCm}cm` : '',
  ];
  return parts.filter(Boolean).join(' · ');
}

function renderRow(row, maxBar) {
  const bar = maxBar > 0 ? Math.round((row.effortBarPct / maxBar) * 100) : 0;
  const kind = row.kind === 'wall' ? 'wall' : 'pawn';
  const depthClass =
    row.childDepthUsed <= 2 && row.reduction >= 2
      ? 'lmr-dispersion__depth--cold'
      : row.childDepthUsed >= row.childDepthFull - 1
        ? 'lmr-dispersion__depth--hot'
        : '';
  return `
    <div class="lmr-dispersion__row" title="${escapeHtml(rowTitle(row))}">
      <span class="lmr-dispersion__move lmr-dispersion__move--${kind}">${escapeHtml(row.move)}</span>
      <div class="lmr-dispersion__bar-wrap">
        <div class="lmr-dispersion__bar" style="width:${bar}%"></div>
      </div>
      <span class="lmr-dispersion__pct">${fmtShare(row.sharePct)}</span>
      <span class="lmr-dispersion__depth ${depthClass}">d${row.childDepthUsed || '?'}</span>
      <span class="lmr-dispersion__cut">${row.reduction > 0 ? `−${row.reduction}` : '—'}</span>
    </div>`;
}

export function renderLmrDispersionPanelHtml(state) {
  if (!state.settings?.showLmrVision || state.settings.uiMode === 'replay') {
    return '';
  }
  if (state.lmrVizLoading) {
    return `<div class="lmr-dispersion lmr-dispersion--loading"><p class="lmr-dispersion__title">LMR effort</p><p class="lmr-dispersion__hint">Loading…</p></div>`;
  }
  if (state.lmrVizError) {
    return `<div class="lmr-dispersion lmr-dispersion--error"><p class="lmr-dispersion__title">LMR effort</p><p class="lmr-dispersion__hint">${escapeHtml(state.lmrVizError)}</p></div>`;
  }
  const viz = state.lmrViz;
  const rows = viz?.dispersion ?? [];
  if (!rows.length) {
    return '';
  }
  const maxBar = Math.max(...rows.map((r) => r.effortBarPct ?? 0), 1);
  const mode = viz.shallow ? 'Plan (pre-search)' : `Search d${viz.searchDepth ?? '?'}`;
  const totalNodes = viz.totalNodes ?? 0;
  const meta = viz.shallow
    ? `${rows.length} moves · planned depth cuts`
    : `${rows.length} moves · ${totalNodes.toLocaleString()} root nodes`;

  return `
    <div class="lmr-dispersion" data-lmr-dispersion>
      <div class="lmr-dispersion__head">
        <p class="lmr-dispersion__title">LMR effort dispersion</p>
        <p class="lmr-dispersion__meta">${escapeHtml(mode)} · ${escapeHtml(meta)}</p>
      </div>
      <div class="lmr-dispersion__legend">
        <span>move</span><span>node share</span><span>%</span><span>child</span><span>cut</span>
      </div>
      <div class="lmr-dispersion__list">
        ${rows.map((row) => renderRow(row, maxBar)).join('')}
      </div>
    </div>`;
}

/** Insert or refresh dispersion panel without full controls re-render. */
export function updateLmrDispersionPanel(container, state) {
  const host = container.querySelector('.controls-card');
  if (!host) {
    return;
  }
  let slot = host.querySelector('[data-lmr-dispersion-root]');
  if (!slot) {
    slot = document.createElement('div');
    slot.dataset.lmrDispersionRoot = '';
    host.querySelector('.toggle-group--board')?.insertAdjacentElement('afterend', slot);
  }
  slot.innerHTML = renderLmrDispersionPanelHtml(state);
}
