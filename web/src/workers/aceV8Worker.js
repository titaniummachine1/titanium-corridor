/**
 * ACE v8 engine in a Web Worker — unmodified extract from quoridor (5).html.
 */

import engineJs from '../vendor/ace-v8/engine.js?raw';
import { algebraicToAceMove, aceMoveToAlgebraic } from '../lib/aceV8Codec.js';

const bootstrap = new Function(
  'postMessage',
  'performance',
  'algebraicToAceMove',
  'aceMoveToAlgebraic',
  `${engineJs}

  var game = new Quoridor();
  var search = new Search(game);

  function pathDistances() {
    search.refreshDist(0);
    return {
      whiteDist: search.dist0[game.pawn[0]],
      blackDist: search.dist1[game.pawn[1]],
    };
  }

  function loadAlgebraicMoves(moves) {
    game.reset();
    for (var i = 0; i < moves.length; i++) {
      game.makeMove(algebraicToAceMove(moves[i]));
    }
  }

  self.onmessage = function (ev) {
    var data = ev.data;
    try {
      loadAlgebraicMoves(data.algebraicMoves || []);
      if (game.winner() >= 0) {
        postMessage({ type: 'error', message: 'position already decided' });
        return;
      }

      var timeMs = Math.max(50, Number(data.timeMs) || 4000);
      var maxDepth = Math.min(30, Math.max(1, Number(data.maxDepth) || 30));
      var result = search.think(timeMs, maxDepth, false);
      var algebraicMove = aceMoveToAlgebraic(result.move);
      var dist = pathDistances();
      var pv = algebraicMove;
      postMessage({
        type: 'bestmove',
        algebraicMove: algebraicMove,
        nodes: result.nodes,
        searchDepth: result.depth,
        rootScore: result.score,
        depthLog: [{
          depth: result.depth,
          score: result.score,
          nodes: result.nodes,
          pv: pv,
        }],
        stoppedBy: 'ace-v8-js',
        mode: 'ace-v8-js',
        profileName: 'ACE v8 (JS HTML)',
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

bootstrap(postMessage, performance, algebraicToAceMove, aceMoveToAlgebraic);
