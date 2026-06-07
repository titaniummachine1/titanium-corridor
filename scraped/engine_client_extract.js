/**
 * Scraped from quoridor-ai.netlify.app (index-xQ3tC4A2.js)
 * WebSocket bridge to remote Ishtar/Ka engines + Redux middleware that drives it.
 *
 * Original minified names restored:
 *   mT  → EngineClient
 *   Lr  → activeEngines (Map)
 *   ru  → requestEngineMoveIfAiTurn
 *   vT  → syncEngineToGameState
 *   R7  → getOrCreateEngine
 *   Pd  → getActiveEngines
 *
 * NOTE: The AI binary is NOT in this file. It runs on quoridor-ai.com servers.
 */

import {
  PlayerType,
  EngineStatus,
  Notation,
  AUTH_TOKEN,
  INFO_LINE_RE,
  BESTMOVE_LINE_RE,
  buildPositionString,
  parseInfoLine,
} from './engine_config_extract.js';

import {
  Direction,
  toAlgebraic,
  parseAlgebraic,
  transformCoordinate,
  isWallAction,
} from './game_logic_extract.js';

// ---------------------------------------------------------------------------
// EngineClient — WebSocket protocol bridge (class mT)
// ---------------------------------------------------------------------------

class EngineClient {
  ws = null;
  sendBuffer = [];
  outstandingMakeMove = 0;
  pondering = false;

  constructor(store, engineSettings) {
    this.store = store;
    this.settings = engineSettings;
  }

  // --- Public API (called by Redux middleware) --------------------------------

  go(rootState) {
    const timeMode = rootState.settings.timeToMove;
    const visits = this.settings.visits?.[timeMode];

    this.outstandingMakeMove++;

    if (Number.isFinite(visits)) {
      this.send(`setoption name visits value ${visits}`);
    }

    this.sendTimeToMoveSettings(timeMode);
    this.send('go');
    this.setEngineStatus(EngineStatus.Searching);
  }

  ponder() {
    if (this.outstandingMakeMove > 0 || this.pondering) {
      return;
    }

    this.send('go ponder');
    this.pondering = true;
    this.setEngineStatus(EngineStatus.Pondering);
  }

  makeMoves(actions) {
    const moves = actions
      .map((action) => this.toEngineAlgebraic(action))
      .join(' ');

    if (moves) {
      this.send(`makemove ${moves}`);
    }

    this.setEngineStatus(EngineStatus.Idle);
  }

  newGame() {
    this.reset();
  }

  setPosition(gameSlice) {
    const position = buildPositionString(gameSlice, this.settings.notation);
    this.send(`setposition ${position}`);
  }

  reset() {
    this.ws?.close();
    this.ws = null;
    this.sendBuffer = [];
    this.outstandingMakeMove = 0;
    this.setEngineStatus(EngineStatus.Idle);
  }

  destroy() {
    this.stop();
    this.ws?.close();
    this.setEngineStatus(EngineStatus.Idle);
  }

  stop() {
    if (!this.pondering) {
      return;
    }

    this.send('stop');
    this.pondering = false;
    this.setEngineStatus(EngineStatus.Idle);
  }

  // --- WebSocket lifecycle ----------------------------------------------------

  connectIfNotConnected() {
    if (this.ws) {
      return;
    }

    const socket = new WebSocket(this.settings.uri);
    this.ws = socket;
    this.setEngineStatus(EngineStatus.Connecting);

    socket.addEventListener('open', () => this.onConnectionOpen());
    socket.addEventListener('message', (event) => this.onMessage(event.data));
    socket.addEventListener('error', () => {
      if (this.ws === socket) {
        this.onConnectionError();
      }
    });
    socket.addEventListener('close', () => {
      if (this.ws === socket) {
        this.onConnectionClose();
      }
    });
  }

  send(command) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(command);
    } else {
      this.sendBuffer.push(command);
    }

    this.connectIfNotConnected();
  }

  onConnectionOpen() {
    this.ws?.send(
      JSON.stringify({ token: AUTH_TOKEN, version: '0.0.0' }),
    );

    this.sendStaticSettings();

    const timeMode = this.store.getState().settings.timeToMove;
    this.sendTimeToMoveSettings(timeMode);

    for (const buffered of this.sendBuffer) {
      this.send(buffered);
    }
    this.sendBuffer = [];
  }

  onConnectionError() {
    this.setEngineStatus(EngineStatus.Error);
  }

  onConnectionClose() {
    this.setEngineStatus(EngineStatus.Error);
  }

  // --- Incoming messages ------------------------------------------------------

  onMessage(rawMessage) {
    if (
      /log Error/i.test(rawMessage) &&
      !/log Error: WARNING:tensorflow/i.test(rawMessage)
    ) {
      this.setEngineStatus(EngineStatus.Error);
      return;
    }

    const infoMatch = INFO_LINE_RE.exec(rawMessage);
    if (infoMatch) {
      this.updateInfo(infoMatch[1]);
      return;
    }

    const bestMoveMatch = BESTMOVE_LINE_RE.exec(rawMessage);
    if (!bestMoveMatch) {
      return;
    }

    this.outstandingMakeMove--;
    this.setEngineStatus(EngineStatus.Idle);

    const state = this.store.getState();
    const isLivePosition = state.game.index === null;
    const isThisEngineActive =
      getCurrentPlayerType(state) === this.settings.key;

    if (
      isThisEngineActive &&
      isLivePosition &&
      this.outstandingMakeMove === 0
    ) {
      const action = this.fromEngineAlgebraic(bestMoveMatch[1]);
      this.store.dispatch(gameActions.takeAction(action));
    }
  }

  // --- Notation conversion ----------------------------------------------------

  toEngineAlgebraic(action) {
    let normalized = action;

    if (
      isWallAction(action) &&
      this.settings.notation === Notation.Glendenning
    ) {
      normalized = {
        ...action,
        coordinate: transformCoordinate(action.coordinate, [Direction.Up]),
      };
    }

    return toAlgebraic(normalized);
  }

  fromEngineAlgebraic(move) {
    const action = parseAlgebraic(move);

    if (
      isWallAction(action) &&
      this.settings.notation === Notation.Glendenning
    ) {
      action.coordinate = transformCoordinate(action.coordinate, [
        Direction.Down,
      ]);
    }

    return action;
  }

  // --- Engine options ---------------------------------------------------------

  sendStaticSettings() {
    if (!this.settings.settings) {
      return;
    }

    for (const [name, value] of Object.entries(this.settings.settings)) {
      if (typeof value === 'string') {
        this.send(`setoption name ${name} value ${value}`);
      }
    }
  }

  sendTimeToMoveSettings(timeMode) {
    if (!this.settings.settings) {
      return;
    }

    for (const [name, value] of Object.entries(this.settings.settings)) {
      if (typeof value !== 'string') {
        this.send(`setoption name ${name} value ${value[timeMode]}`);
      }
    }
  }

  setEngineStatus(status) {
    this.store.dispatch(
      engineSliceActions.setEngineStatus({
        name: this.settings.key,
        status,
      }),
    );
  }

  updateInfo(infoLine) {
    if (!this.settings.displayEval) {
      return;
    }

    const info = parseInfoLine(infoLine);

    if (info.pv) {
      info.pv = info.pv
        .split(' ')
        .map((move) => this.fromEngineAlgebraic(move));
    }

    this.store.dispatch(analysisActions.updateInfo(info));
  }
}

// ---------------------------------------------------------------------------
// Engine registry & Redux middleware
// ---------------------------------------------------------------------------

const activeEngines = new Map();

function getActiveEngines() {
  return [...activeEngines.values()];
}

function getOrCreateEngine(engineKey, store) {
  if (activeEngines.has(engineKey)) {
    console.error(`Engine ${engineKey} already exists!`);
    activeEngines.get(engineKey).reset();
  } else {
    const config = store
      .getState()
      .settings.engines.find((engine) => engine.key === engineKey);
    activeEngines.set(engineKey, new EngineClient(store, config));
  }

  return activeEngines.get(engineKey);
}

function getCurrentPlayerType(state) {
  return state.settings.player[state.game.currentState.playerToMove - 1];
}

function isStandardBoard(state) {
  return state.game.numCols === 9 && state.game.numRows === 9;
}

function isPlayerTurnAndLive(state, playerNum) {
  const atLiveIndex = state.game.index === null;
  const notOver = !state.game.isTerminal;
  return state.game.displayState.playerToMove === playerNum && notOver && atLiveIndex;
}

/** If current player is an AI, tell that engine to search. */
function requestEngineMoveIfAiTurn(state) {
  const playerToMove = state.game.currentState.playerToMove;
  const playerType = state.settings.player[playerToMove - 1];

  if (playerType === PlayerType.Human) {
    return;
  }

  if (state.game.isTerminal) {
    return;
  }

  activeEngines.get(playerType).go(state);
}

/** Replay game history on the engine after undo/rewind. */
function syncEngineToGameState(state, engine) {
  engine.newGame();

  if (state.game.startingState.moveNumber === 1) {
    engine.makeMoves(state.game.actions);
    return;
  }

  engine.setPosition(state.game);

  if (state.game.index !== null) {
    const futureMoves = state.game.actions.slice(state.game.index + 1);
    engine.makeMoves(futureMoves);
  }
}

// Stubs for Redux slices referenced by the original bundle
const gameActions = { takeAction: { type: 'game/takeAction' } };
const engineSliceActions = { setEngineStatus: { type: 'engine/setEngineStatus' } };
const analysisActions = { updateInfo: { type: 'analysis/updateInfo' } };

/**
 * Register listeners that keep engines in sync with the game.
 * In the original app this uses RTK listener middleware (`startListening`).
 */
function registerEngineMiddleware(listener) {
  listener({
    matcher: matchAny(
      gameActions.setBoardSize,
      gameActions.setNumWallsForPlayer,
    ),
    effect: (_action, api) => {
      const state = api.getState();
      if (isStandardBoard(state)) {
        return;
      }

      state.settings.player.forEach((playerType, index) => {
        if (playerType !== PlayerType.Human) {
          api.dispatch(
            settingsActions.setPlayer({
              playerNum: index + 1,
              key: PlayerType.Human,
            }),
          );
        }
      });
    },
  });

  listener({
    actionCreator: gameActions.takeAction,
    effect: (action, api) => {
      const state = api.getState();
      const wasRewound = api.getOriginalState().game.index !== null;

      if (wasRewound) {
        getActiveEngines().forEach((engine) => syncEngineToGameState(state, engine));
      } else {
        getActiveEngines().forEach((engine) => engine.makeMoves([action.payload]));
      }

      requestEngineMoveIfAiTurn(state);
    },
  });

  listener({
    actionCreator: settingsActions.setPlayer,
    effect: (action, api) => {
      const state = api.getState();
      const { key: engineKey, playerNum } = action.payload;

      if (!activeEngines.has(engineKey) && engineKey !== PlayerType.Human) {
        getOrCreateEngine(engineKey, api).makeMoves(state.game.actions);
      }

      for (const [key, engine] of activeEngines.entries()) {
        if (!state.settings.player.includes(key)) {
          engine.destroy();
          activeEngines.delete(key);
        }
      }

      if (
        state.game.currentState.playerToMove === playerNum &&
        engineKey !== PlayerType.Human
      ) {
        requestEngineMoveIfAiTurn(state);
      }
    },
  });

  listener({
    matcher: matchAny(gameActions.undo, gameActions.setGameState),
    effect: (_action, api) => {
      const state = api.getState();
      getActiveEngines().forEach((engine) => syncEngineToGameState(state, engine));
      requestEngineMoveIfAiTurn(state);
    },
  });

  listener({
    actionCreator: gameActions.newGame,
    effect: (_action, api) => {
      const state = api.getState();
      getActiveEngines().forEach((engine) => engine.newGame());
      requestEngineMoveIfAiTurn(state);
    },
  });

  listener({
    matcher: matchAny(gameActions.forward, gameActions.setIndex),
    effect: (_action, api) => {
      const state = api.getState();
      if (state.game.index === null) {
        requestEngineMoveIfAiTurn(state);
      }
    },
  });
}

// Placeholder helpers matching original Redux Toolkit patterns
const settingsActions = { setPlayer: { type: 'settings/setPlayer' } };
function matchAny(..._creators) {
  return () => false;
}

export {
  EngineClient,
  activeEngines,
  getActiveEngines,
  getOrCreateEngine,
  requestEngineMoveIfAiTurn,
  syncEngineToGameState,
  registerEngineMiddleware,
  isPlayerTurnAndLive,
};
