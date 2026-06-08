/**
 * Ishtar engine over WebSocket (Glendenning notation).
 */

const ISHTAR_URI = 'wss://quoridor-ai.com/ishtar-v3';
const AUTH_TOKEN = 'rbt_token_*';

export const ISHTAR_VISITS = {
  intuition: 2,
  short: 3_200,
  medium: 200_000,
  long: 1_000_000,
};

/** Official → Glendenning (walls only — row +1). */
export function toGlendenningAlgebraic(official) {
  if (official.length <= 2) {
    return official;
  }
  const col = official[0];
  const row = Number(official[1]);
  const suffix = official.slice(2);
  return `${col}${row + 1}${suffix}`;
}

export function requestIshtarMove(algebraicHistory, { visits = ISHTAR_VISITS.short, timeoutMs = 180_000 } = {}) {
  const glendenningMoves = algebraicHistory.map(toGlendenningAlgebraic);

  return new Promise((resolve, reject) => {
    const ws = new WebSocket(ISHTAR_URI);
    let settled = false;

    function finish(err, move) {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      try {
        ws.close();
      } catch {
        // ignore
      }
      if (err) {
        reject(err);
      } else {
        resolve(move);
      }
    }

    const timer = setTimeout(() => {
      finish(new Error(`Ishtar timeout after ${timeoutMs}ms`));
    }, timeoutMs);

    ws.addEventListener('open', () => {
      ws.send(JSON.stringify({ token: AUTH_TOKEN, version: '0.0.0' }));
      ws.send('setoption name display value false');
      ws.send('setoption name alternative_action_threshold value 0.1');
      if (glendenningMoves.length > 0) {
        ws.send(`makemove ${glendenningMoves.join(' ')}`);
      }
      ws.send(`setoption name visits value ${visits}`);
      ws.send('go');
    });

    ws.addEventListener('message', (event) => {
      const message = event.data.toString();
      if (/log Error/i.test(message) && !/tensorflow/i.test(message)) {
        finish(new Error(message));
        return;
      }
      if (message.startsWith('bestmove')) {
        const glendenning = message.trim().split(/\s+/)[1];
        if (!glendenning) {
          finish(new Error(`Ishtar empty bestmove: ${message}`));
          return;
        }
        if (glendenning.length <= 2) {
          finish(null, glendenning);
          return;
        }
        const col = glendenning[0];
        const row = Number(glendenning[1]) - 1;
        const suffix = glendenning.slice(2);
        finish(null, `${col}${row}${suffix}`);
      }
    });

    ws.addEventListener('error', () => {
      finish(new Error('Ishtar WebSocket error'));
    });
  });
}
