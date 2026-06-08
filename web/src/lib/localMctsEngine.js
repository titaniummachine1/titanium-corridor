/**
 * Gorisanson MCTS in a Web Worker. Titanium uses titaniumRustClient.js (Rust).
 */

import GorisansonWorker from '../workers/gorisansonWorker.js?worker';
import { parseAlgebraic, toAlgebraic } from './gameLogic.js';
import { LOCAL_VISITS_RANGE, clampVisits, uctFromStrengthLevel } from './timeControl.js';

export class LocalMctsEngineClient {
  constructor(engineConfig, { resolveUct, WorkerClass = GorisansonWorker } = {}) {
    this.config = engineConfig;
    this.WorkerClass = WorkerClass;
    this.resolveUct = resolveUct ?? (() => engineConfig.uctConst ?? 0.2);
    this.worker = null;
    this.algebraicMoves = [];
    this.isPondering = false;
    this.pendingRequest = null;
  }

  ensureWorker() {
    if (this.worker) {
      return;
    }

    this.worker = new this.WorkerClass();

    this.worker.onmessage = (event) => {
      const data = event.data;
      const pending = this.pendingRequest;
      if (!pending) {
        return;
      }

      if (data.type === 'progress' || data.type === 'depth') {
        pending.onProgress?.(data);
        return;
      }
      if (data.type === 'error') {
        this.pendingRequest = null;
        this.setStatus('error');
        pending.onError?.(new Error(data.message ?? 'Worker error'));
        return;
      }
      if (data.type === 'bestmove') {
        const elapsed = performance.now() - pending.started;
        this.pendingRequest = null;
        this.setStatus('idle');
        pending.onInfo?.({
          time: elapsed,
          simulations: data.simulations,
          stoppedBy: data.stoppedBy,
          searchDepth: data.searchDepth,
          depthLog: data.depthLog,
          nodes: data.nodes,
          rootScore: data.rootScore,
          whiteDist: data.whiteDist,
          blackDist: data.blackDist,
          lmrReSearches: data.lmrReSearches,
          aspirationFails: data.aspirationFails,
          profileName: data.profileName,
          progress: 1,
        });
        if (!data.algebraicMove) {
          pending.onError?.(new Error('Worker returned no algebraic move'));
          return;
        }
        const action = parseAlgebraic(data.algebraicMove);
        this.algebraicMoves.push(data.algebraicMove);
        pending.onBestMove?.(action);
      }
    };

    this.worker.onerror = (err) => {
      this.pendingRequest = null;
      this.setStatus('error');
      this.onError?.(err);
    };
  }

  /**
   * Future: node-cap-only MCTS on predicted opponent reply (no wall clock).
   * @see docs/video/09-pondering-prep.md
   */
  ponder() {
    this.isPondering = false;
  }

  stopPonder() {
    if (!this.isPondering) {
      return;
    }
    this.worker?.terminate();
    this.worker = null;
    this.isPondering = false;
    this.setStatus('idle');
  }

  destroy() {
    this.worker?.terminate();
    this.worker = null;
    this.algebraicMoves = [];
    this.setStatus('idle');
  }

  resetConnection() {
    this.destroy();
    this.algebraicMoves = [];
  }

  makeMoves(actions) {
    for (const action of actions) {
      this.algebraicMoves.push(toAlgebraic(action));
    }
    this.setStatus('idle');
  }

  requestMove({ aiSettings, moveHistory, isFreshGame }) {
    if (isFreshGame) {
      this.algebraicMoves = [];
    } else if (moveHistory?.length) {
      this.algebraicMoves = moveHistory.map(toAlgebraic);
    }

    const timeMs = Math.round((aiSettings?.wallClockSeconds ?? 3) * 1000);
    const maxSimulations = clampVisits(aiSettings?.visitsBudget ?? LOCAL_VISITS_RANGE.default);
    const uctConst = this.resolveUct(aiSettings);

    this.setStatus('searching');
    const started = performance.now();
    this.ensureWorker();

    this.pendingRequest = {
      started,
      onProgress: (data) => {
        if (data.type === 'depth') {
          this.onInfo?.({
            thinking: true,
            searchDepth: data.depth,
            nodes: data.nodes,
            depthLog: [{ depth: data.depth, score: data.score, nodes: data.nodes }],
          });
          return;
        }
        this.onInfo?.({
          thinking: true,
          progress: data.value,
          simulations: data.simulations,
        });
      },
      onInfo: (info) => this.onInfo?.(info),
      onBestMove: (action) => this.onBestMove?.(action),
      onError: (err) => this.onError?.(err),
    };

    const payload = {
      algebraicMoves: this.algebraicMoves,
      timeMs,
      maxSimulations,
      uctConst,
    };
    this.worker.postMessage(payload);
  }

  setStatus(status) {
    this.onStatus?.(status);
  }
}

export class GorisansonEngineClient extends LocalMctsEngineClient {
  constructor(engineConfig) {
    super(engineConfig, {
      resolveUct: () => engineConfig.uctConst ?? 0.2,
    });
  }
}

export { TitaniumEngineClient } from './titaniumRustClient.js';
