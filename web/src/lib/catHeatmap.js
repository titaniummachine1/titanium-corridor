/** CAT v3 heat → subtle board overlays (never solid black bars on walls). */

/** Unreachable sealed square — only case that gets a dark skip overlay. */
export function isSquareSkipped(reachable) {
  return reachable === false;
}

const DEFAULT_COLD_CM = 60;
const DEFAULT_HOT_CM = 160;
// Per-player corridor ceiling: CAT_CORRIDOR_CM + BOTTLENECK_BONUS_CM (engine constants).
const DEFAULT_MAX_CM = 240;

// Fixed normalized position of the hot threshold on the color ramp.
// cold → 0, hot → 0.65, max → 1. Piecewise-linear with engine-true anchors:
// the same cm value ALWAYS renders the same color, and crossing CAT_HOT_CM
// (tactical / no-LMR) is always the same visual jump regardless of maxCm.
const HOT_ANCHOR_T = 0.65;

/**
 * Engine-true heat → normalized 0..1 ramp position. Anchored to the engine's
 * own thresholds (coldCm LMR cutoff, hotCm tactical cutoff, maxCm ceiling) —
 * never per-position normalization.
 */
export function catHeatT(heat, scale = {}) {
  const coldCm = scale.coldCm ?? DEFAULT_COLD_CM;
  const hotCm = Math.max(scale.hotCm ?? DEFAULT_HOT_CM, coldCm + 1);
  const maxCm = Math.max(scale.maxCm ?? DEFAULT_MAX_CM, hotCm + 1);
  if (!Number.isFinite(heat) || heat < coldCm) {
    return 0;
  }
  if (heat >= hotCm) {
    return HOT_ANCHOR_T + (1 - HOT_ANCHOR_T) * Math.min(1, (heat - hotCm) / (maxCm - hotCm));
  }
  return HOT_ANCHOR_T * ((heat - coldCm) / (hotCm - coldCm));
}

/**
 * Engine-true heat → color. Below `coldCm` (LMR fringe) there is no tint —
 * the raw cm value is still shown as a number on the square.
 *
 * @returns {{ fill: string, opacity: number } | null}
 */
export function catSquareOverlay(heat, reachable, scale = {}) {
  if (isSquareSkipped(reachable)) {
    return null;
  }
  if (!Number.isFinite(heat) || heat <= 0) {
    return null;
  }
  const coldCm = scale.coldCm ?? DEFAULT_COLD_CM;
  // Below CAT_COLD_CM: search treats as cold fringe (extra LMR) — no board tint.
  if (heat < coldCm) {
    return null;
  }
  const t = catHeatT(heat, scale);
  // Yellow (55°) → orange → red (0°); alpha ramps so even the coolest warm
  // square reads against the background instead of vanishing into it.
  const hue = Math.round(55 * (1 - t));
  const sat = Math.round(88 + 8 * t);
  const light = Math.round(56 - 8 * t);
  const alpha = Math.min(0.62, 0.3 + 0.28 * t);
  return {
    fill: `hsla(${hue}, ${sat}%, ${light}%, ${alpha.toFixed(2)})`,
    opacity: 1,
  };
}

/** Outline color for searchable wall hints (not filled bars). */
export function catWallOutlineColor(heat, scale = {}) {
  if (!Number.isFinite(heat) || heat <= 0) {
    return 'rgba(120, 115, 105, 0.55)';
  }
  const overlay = catSquareOverlay(heat, true, scale);
  if (!overlay) {
    return 'rgba(120, 115, 105, 0.55)';
  }
  return overlay.fill.replace(/,\s*[\d.]+\)$/, ', 0.85)');
}

/**
 * @param {string[]} algebraicMoves
 */
export async function fetchCatSnapshot(algebraicMoves) {
  const res = await fetch('/api/titanium/cat', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ moves: algebraicMoves }),
  });
  const data = await res.json();
  if (!res.ok || data.error) {
    throw new Error(data.error ?? `CAT request failed (${res.status})`);
  }
  return data;
}

/**
 * @param {Array<{alg: string, heat: number, search?: boolean, skip?: boolean, pruned?: boolean}>} walls
 * @returns {Map<string, {heat: number, search: boolean, skip: boolean}>}
 */
export function indexCatWalls(walls) {
  const map = new Map();
  for (const entry of walls ?? []) {
    if (!entry?.alg) {
      continue;
    }
    const skip = entry.skip ?? entry.pruned ?? false;
    const search = entry.search ?? !skip;
    map.set(entry.alg, {
      heat: entry.heat ?? 0,
      search,
      skip,
    });
  }
  return map;
}

/** Engine row/col 0..8 → flat index in squares[81]. */
export function catSquareIndex(engineRow, engineCol) {
  return engineRow * 9 + engineCol;
}
