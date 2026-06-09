# CAT vision overlay — web UI spec

**Purpose:** Debug overlay showing the same corridor heat the engine uses for ordering/LMR — not a separate visualization layer.

---

## Enabling

- **Analysis** or **Play** mode → board toggles → **CAT** checkbox
- Fetches `POST /api/titanium/cat` with current move list
- Not shown in **Replay** mode (scrubbing only)

---

## Square display

| Element         | Meaning                                                    |
| --------------- | ---------------------------------------------------------- |
| **Number (cm)** | Raw engine heat for that square — exactly what search sees |
| No number       | `heat == 0` or unreachable                                 |
| Warm tint       | `heat ≥ 60` (`CAT_COLD_CM`) — corridor fringe, extra LMR   |
| Hot tint        | `heat ≥ 160` (`CAT_HOT_CM`) — tactical, no LMR             |
| Dark overlay    | Square unreachable (sealed void)                           |

### Color ramp (fixed anchors — not per-position normalize)

Piecewise linear on engine thresholds so the same cm always renders the same color:

- `coldCm` (60) → no tint (number only if > 0)
- `hotCm` (160) → 65% along yellow→red ramp
- `maxCm` (240) → full red (`CAT_CORRIDOR_CM + BOTTLENECK_BONUS_CM`)

Implementation: `web/src/lib/catHeatmap.js` → `catHeatT()`, `catSquareOverlay()`.

**Removed:** γ sharpness slider (was display-only gamma; confused debugging).

---

## Wall hints

- Orange outline on **searchable** legal walls (`search: true`, `skip: false`)
- Hover shows `CAT {heat} cm`
- Placed walls: no hint layer

---

## Status line

Controls panel when CAT on: `W{whiteDist} B{blackDist}` from engine snapshot.

---

## Hint card (first enable)

Explains: numbers = raw cm; ≥60 warm; ≥160 hot; dark = unreachable; outlines = searchable walls.

Dismiss with **Got it** → `catHintDismissed` in local state.

---

## API payload (`cat_snapshot_json`)

```json
{
  "squares": [81 × u16 heat],
  "reachable": [81 × 0|1],
  "walls": [{ "alg", "heat", "search", "skip" }, ...],
  "whiteDist", "blackDist",
  "hotCm": 160, "coldCm": 60, "maxCm": 240
}
```

Display squares use **per-player max** corridor heat (`build_corridor_display_squares`), not summed search table — avoids mid-game full-board flood tint.
