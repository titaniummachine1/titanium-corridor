import { GameSession } from './gameSession.js';
import { EngineClient } from '../lib/engineClient.js';
import { GorisansonEngineClient } from '../lib/gorisansonEngine.js';
import {
  PlayerType,
  TimeToMove,
  EngineStatus,
} from '../lib/engineConfig.js';
import {
  TIME_PRESETS,
  getAllEngineConfigs,
  getPlayerOptionGroups,
  flattenPlayerOptions,
  describeTimeBudget,
  describeActiveSearchInfo,
} from '../lib/playerRegistry.js';

export class AppController {
  constructor() {
    this.session = new GameSession();
    this.engines = new Map();
    this.engineConfigs = getAllEngineConfigs();

    this.settings = {
      players: [PlayerType.Human, PlayerType.IshtarV3],
      timeToMove: TimeToMove.Short,
      rotateBoard: false,
      displayCoordinates: true,
      displayRemainingWalls: true,
      displayEvalBar: true,
    };

    this.engineStatus = {};
    this.searchInfo = {};
    this.eval = { score: 0.5, p1: 0.5, pv: [] };
    this.aiThinking = false;

    this.session.subscribe(() => this.onSessionChange());
  }

  getState() {
    return {
      ...this.session.getSnapshot(),
      settings: { ...this.settings },
      engineStatus: { ...this.engineStatus },
      eval: { ...this.eval },
      aiThinking: this.aiThinking,
      timePresets: TIME_PRESETS,
      playerOptionGroups: getPlayerOptionGroups(),
      playerOptions: flattenPlayerOptions(getPlayerOptionGroups()),
      timeBudgetHint: describeTimeBudget(
        this.settings.players,
        this.settings.timeToMove,
        this.engineConfigs,
      ),
      searchInfoLine: describeActiveSearchInfo(
        this.settings.players,
        this.searchInfo,
        this.engineConfigs,
      ),
    };
  }

  onChange = null;

  setPlayer(playerNum, playerType) {
    if (playerType === PlayerType.Titanium || playerType === PlayerType.Pavlosdais) {
      return;
    }
    this.settings.players[playerNum - 1] = playerType;
    this.onChange?.();
    this.maybeRequestAiMove();
  }

  setTimeToMove(timeMode) {
    this.settings.timeToMove = Number(timeMode);
    this.onChange?.();
  }

  toggleRotateBoard() {
    this.settings.rotateBoard = !this.settings.rotateBoard;
    this.onChange?.();
  }

  toggleDisplayCoordinates() {
    this.settings.displayCoordinates = !this.settings.displayCoordinates;
    this.onChange?.();
  }

  toggleDisplayRemainingWalls() {
    this.settings.displayRemainingWalls = !this.settings.displayRemainingWalls;
    this.onChange?.();
  }

  toggleDisplayEvalBar() {
    this.settings.displayEvalBar = !this.settings.displayEvalBar;
    this.onChange?.();
  }

  newGame() {
    this.aiThinking = false;
    this.eval = { score: 0.5, p1: 0.5, pv: [] };
    for (const engine of this.engines.values()) {
      engine.resetConnection();
    }
    this.session.reset();
    this.onChange?.();
    this.maybeRequestAiMove();
  }

  undo() {
    if (this.aiThinking) {
      return;
    }
    this.session.undo();
    for (const engine of this.engines.values()) {
      engine.resetConnection();
    }
    this.onChange?.();
    this.maybeRequestAiMove();
  }

  tryAction(action) {
    if (this.aiThinking || !this.session.isHumanTurn(this.settings.players)) {
      return;
    }

    const applied = this.session.applyAction(action);
    if (!applied) {
      return;
    }

    this.syncEnginesAfterHumanMove(action);
    this.onChange?.();
    this.maybeRequestAiMove();
  }

  onSessionChange() {
    this.onChange?.();
  }

  createEngineClient(config) {
    if (config.kind === 'local') {
      return new GorisansonEngineClient(config);
    }
    return new EngineClient(config);
  }

  getEngine(playerType) {
    if (playerType === PlayerType.Human) {
      return null;
    }

    if (!this.engines.has(playerType)) {
      const config = this.engineConfigs.find((entry) => entry.key === playerType);
      if (!config || config.disabled) {
        return null;
      }

      const engine = this.createEngineClient(config);
      engine.onStatus = (status) => {
        const prev = this.engineStatus[playerType];
        this.engineStatus[playerType] = status;
        if (prev !== status) {
          this.onChange?.();
        }
      };
      engine.onInfo = (info) => {
        this.searchInfo[playerType] = { ...this.searchInfo[playerType], ...info };
        // Progress-only updates (local MCTS) must not re-render the board every tick.
        if (info.progress !== undefined && info.p1 === undefined && !info.pv) {
          return;
        }
        if (info.p1 !== undefined) {
          this.eval.p1 = info.p1;
          this.eval.score = info.score ?? info.p1;
        }
        if (info.pv) {
          this.eval.pv = info.pv;
        }
        this.onChange?.();
      };
      engine.onError = () => {
        this.aiThinking = false;
        this.onChange?.();
      };
      this.engines.set(playerType, engine);
    }

    return this.engines.get(playerType);
  }

  syncEnginesAfterHumanMove(action) {
    for (const playerType of this.settings.players) {
      if (playerType === PlayerType.Human) {
        continue;
      }
      const engine = this.getEngine(playerType);
      engine?.makeMoves([action]);
    }
  }

  maybeRequestAiMove() {
    if (this.session.winner) {
      this.aiThinking = false;
      return;
    }

    const playerType = this.session.getCurrentPlayerType(this.settings.players);
    if (playerType === PlayerType.Human) {
      this.aiThinking = false;
      return;
    }

    const engine = this.getEngine(playerType);
    if (!engine) {
      this.aiThinking = false;
      return;
    }

    this.aiThinking = true;
    this.onChange?.();

    engine.onBestMove = (action) => {
      this.aiThinking = false;
      if (!this.session.winner) {
        this.session.applyAction(action);
        this.onChange?.();
        this.maybeRequestAiMove();
      }
    };

    const moveHistory = this.session.actions;
    const isFreshGame = moveHistory.length === 0;

    engine.requestMove({
      timeMode: this.settings.timeToMove,
      gameSnapshot: this.session.getEngineSnapshot(),
      moveHistory,
      isFreshGame,
    });
  }
}
