/**
 * Gorisanson MCTS in a Web Worker — keeps UI responsive.
 * Stops on wall clock or rollout cap (whichever first) and returns best move in tree.
 */

import gameJs from '../../../_vendor/quoridor-mcts/src/js/game.js?raw';
import aiJs from '../../../_vendor/quoridor-mcts/src/js/ai.js?raw';

const bootstrap = new Function(
  'postMessage',
  'performance',
  `${gameJs}\n${aiJs}\n
  function chooseOpeningPawnMove(game) {
    if (game.turn >= 2) {
      return null;
    }
    const nextPosition = AI.chooseShortestPathNextPawnPosition(game);
    const pawnMoveTuple = nextPosition.getDisplacementPawnMoveTupleFrom(game.pawnOfTurn.position);
    if (pawnMoveTuple[1] === 0) {
      return [[nextPosition.row, nextPosition.col], null, null];
    }
    return null;
  }

  function fallbackMove(game) {
    const nextPosition = AI.chooseShortestPathNextPawnPosition(game);
    const pawnMoveTuple = nextPosition.getDisplacementPawnMoveTupleFrom(game.pawnOfTurn.position);
    if (pawnMoveTuple[1] === 0) {
      return [[nextPosition.row, nextPosition.col], null, null];
    }
    const valids = game.getArrOfValidNextPositionTuples();
    if (valids.length > 0) {
      return [[valids[0][0], valids[0][1]], null, null];
    }
    const walls = game.getArrOfProbableValidNoBlockNextHorizontalWallPositions();
    if (walls.length > 0) {
      return [null, walls[0], null];
    }
    const verts = game.getArrOfProbableValidNoBlockNextVerticalWallPositions();
    if (verts.length > 0) {
      return [null, null, verts[0]];
    }
    return null;
  }

  function pickBestMoveFromTree(mcts, game) {
    if (mcts.root.children.length > 0) {
      const best = mcts.selectBestMove();
      if (best && best.move) {
        return best.move;
      }
    }
    return fallbackMove(game);
  }

  function searchForTime(game, uctConst, timeMs, maxSimulations) {
    const opening = chooseOpeningPawnMove(game);
    if (opening) {
      return { move: opening, simulations: 0, stoppedBy: 'opening' };
    }

    const mcts = new MonteCarloTreeSearch(game, uctConst);
    const started = performance.now();
    const deadline = started + timeMs;
    const batchSize = 50;
    let simulations = 0;
    let tick = 0;
    const simCap =
      Number.isFinite(maxSimulations) && maxSimulations > 0 ? maxSimulations : Infinity;

    while (performance.now() < deadline && simulations < simCap) {
      const remainingMs = deadline - performance.now();
      const remainingSims = simCap - simulations;
      const batch = Math.min(remainingMs < 250 ? 1 : batchSize, remainingSims);
      if (batch <= 0) {
        break;
      }

      mcts.search(batch);
      simulations += batch;
      tick += 1;
      if (tick % 5 === 0) {
        const elapsed = performance.now() - started;
        postMessage({ type: 'progress', value: Math.min(0.99, elapsed / timeMs), simulations });
      }
    }

    const stoppedBy = simulations >= simCap ? 'visits' : 'time';
    const move = pickBestMoveFromTree(mcts, game);
    if (!move) {
      throw new Error('no legal move');
    }
    return { move, simulations, stoppedBy };
  }

  return { Game, AI, searchForTime };
  `,
);

const { Game, AI, searchForTime } = bootstrap(
  (msg) => {
    if (typeof msg === 'number') {
      self.postMessage({ type: 'progress', value: msg });
    }
  },
  performance,
);

self.onmessage = (event) => {
  const { gorisansonMoves, simulations, timeMs, maxSimulations, uctConst } = event.data;
  const game = new Game(true);
  for (const move of gorisansonMoves) {
    game.doMove(move, true);
  }

  if (game.winner !== null) {
    self.postMessage({ type: 'error', message: 'terminal position' });
    return;
  }

  if (Number.isFinite(timeMs) && timeMs > 0) {
    try {
      const result = searchForTime(game, uctConst ?? 0.2, timeMs, maxSimulations);
      self.postMessage({
        type: 'bestmove',
        move: result.move,
        simulations: result.simulations,
        stoppedBy: result.stoppedBy,
        timeMs,
      });
    } catch (err) {
      self.postMessage({ type: 'error', message: err.message ?? String(err) });
    }
    return;
  }

  const ai = new AI(simulations, uctConst, false, true);
  const move = ai.chooseNextMove(game);
  self.postMessage({ type: 'bestmove', move, simulations });
};
