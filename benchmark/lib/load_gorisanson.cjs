const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '../..');
const gameJs = fs.readFileSync(
  path.join(root, '_vendor/quoridor-mcts/src/js/game.js'),
  'utf8',
);
const aiJs = fs.readFileSync(
  path.join(root, '_vendor/quoridor-mcts/src/js/ai.js'),
  'utf8',
);

// game.js uses top-level classes in strict mode — Function() exposes them via return.
const factory = new Function(
  'console',
  `${gameJs}\n${aiJs}\nreturn { Game, Board, Pawn, PawnPosition, AI, MonteCarloTreeSearch };`,
);

module.exports = factory(console);
