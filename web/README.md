# Quoridor AI — reverse-engineered clone

Playable web app rebuilt from scraped [quoridor-ai.netlify.app](https://quoridor-ai.netlify.app) code.

## Stack

- **Vite** + vanilla JS modules
- **Rules engine** — `src/lib/gameLogic.js` (scraped `p9` class)
- **Engine protocol** — `src/lib/engineConfig.js` + `src/lib/engineClient.js`
- **AI** — remote WebSocket at `wss://quoridor-ai.com/*` (same as original)
- **Titanium** — Gorisanson-style MCTS in-browser (strength + time + rollout cap)

## Run

```bash
npm install
npm run dev
```

## Build

```bash
npm run build
npm run preview
```

## Features

- 9×9 Quoridor board with wall placement
- Human / Gorisanson MCTS / **Titanium (MCTS)** / Ishtar / Ka per player
- Per-player AI settings (MCTS time+visits, remote strength+time)
- Eval bar, coordinates, wall count, board rotate
- Undo, new game
