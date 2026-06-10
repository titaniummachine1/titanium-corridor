/**
 * Quoridor v3 αβ engine in a Web Worker — vendored from quoridor.html.
 */

import engineJs from '../vendor/quoridor-v3/engine.js?raw';
import { algebraicToV3Move, v3MoveToAlgebraic } from '../lib/quoridorV3Codec.js';

const bootstrap = new Function(
  'postMessage',
  'performance',
  'algebraicToV3Move',
  'v3MoveToAlgebraic',
  `${engineJs}

  var game = new Quoridor();
  var search = new Search(game);

  function pathDistances() {
    search.refreshDist(0);
    var d0 = search.dist0[game.pawn[0]];
    var d1 = search.dist1[game.pawn[1]];
    return { whiteDist: d0, blackDist: d1 };
  }

  function loadAlgebraicMoves(moves) {
    game.reset();
    for (var i = 0; i < moves.length; i++) {
      game.makeMove(algebraicToV3Move(moves[i]));
    }
  }

  self.onmessage = function (ev) {
    var data = ev.data;
    try {
      loadAlgebraicMoves(data.algebraicMoves || []);
      var winner = game.winner();
      if (winner >= 0) {
        postMessage({ type: 'error', message: 'position already decided' });
        return;
      }

      var timeMs = Math.max(50, Number(data.timeMs) || 1500);
      var maxDepth = Math.min(30, Math.max(3, Number(data.maxDepth) || 30));
      var result = search.think(timeMs, maxDepth);
      var algebraicMove = v3MoveToAlgebraic(result.move);
      var legal = game.legalMoves();
      if (legal.indexOf(result.move) < 0) {
        algebraicMove = v3MoveToAlgebraic(legal[0]);
      }
      var dist = pathDistances();
      postMessage({
        type: 'bestmove',
        algebraicMove: algebraicMove,
        nodes: result.nodes,
        searchDepth: result.depth,
        rootScore: result.score,
        depthLog: [{ depth: result.depth, score: result.score, nodes: result.nodes }],
        stoppedBy: 'minimax',
        mode: 'minimax',
        profileName: 'Quoridor v3 αβ',
        whiteDist: dist.whiteDist,
        blackDist: dist.blackDist,
        ms: result.ms,
      });
    } catch (err) {
      postMessage({ type: 'error', message: String(err?.message || err) });
    }
  };
`,
);

bootstrap(postMessage, performance, algebraicToV3Move, v3MoveToAlgebraic);
