# Quoridor AI — reverse-engineered clone

Playable web app rebuilt from scraped [quoridor-ai.netlify.app](https://quoridor-ai.netlify.app) code.

## Stack

- **Vite** + vanilla JS modules
- **Rules engine** — `src/lib/gameLogic.js` (scraped `p9` class)
- **Engine protocol** — `src/lib/engineConfig.js` + `src/lib/engineClient.js`
- **AI** — remote WebSocket at `wss://quoridor-ai.com/*` (same as original)

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
- Human / Ishtar / Ka per player
- AI time presets (Intuition → Long)
- Eval bar, coordinates, wall count, board rotate
- Undo, new game
