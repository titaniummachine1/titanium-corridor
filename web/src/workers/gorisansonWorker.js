/**
 * Gorisanson MCTS in a Web Worker — keeps UI responsive.
 */

import gameJs from '../../../_vendor/quoridor-mcts/src/js/game.js?raw';
import aiJs from '../../../_vendor/quoridor-mcts/src/js/ai.js?raw';

const bootstrap = new Function(
  'postMessage',
  `${gameJs}\n${aiJs}\nreturn { Game, AI };`,
);

const { Game, AI } = bootstrap((msg) => {
  if (typeof msg === 'number') {
    self.postMessage({ type: 'progress', value: msg });
  }
});

self.onmessage = (event) => {
  const { gorisansonMoves, simulations, uctConst } = event.data;
  const game = new Game(true);
  for (const move of gorisansonMoves) {
    game.doMove(move, true);
  }

  if (game.winner !== null) {
    self.postMessage({ type: 'error', message: 'terminal position' });
    return;
  }

  const ai = new AI(simulations, uctConst, false, true);
  const move = ai.chooseNextMove(game);
  self.postMessage({ type: 'bestmove', move });
};
