/**
 * Ka engine over WebSocket — for terminal head-to-head vs local hybrid search.
 */

const KA_URI = 'wss://quoridor-ai.com/ka';
const AUTH_TOKEN = 'rbt_token_*';

export const KA_VISITS = {
  intuition: 1,
  short: 1_000,
  medium: 5_000,
  long: 20_000,
};

/**
 * @param {string[]} algebraicHistory — e.g. ['e2', 'e8', 'e3']
 * @param {{ visits?: number, timeoutMs?: number }} options
 * @returns {Promise<string>} algebraic move from Ka (official notation)
 */
export function requestKaMove(algebraicHistory, { visits = KA_VISITS.short, timeoutMs = 120_000 } = {}) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(KA_URI);
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
      finish(new Error(`Ka timeout after ${timeoutMs}ms`));
    }, timeoutMs);

    ws.addEventListener('open', () => {
      ws.send(JSON.stringify({ token: AUTH_TOKEN, version: '0.0.0' }));
      if (algebraicHistory.length > 0) {
        ws.send(`makemove ${algebraicHistory.join(' ')}`);
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
        const move = message.trim().split(/\s+/)[1];
        if (!move) {
          finish(new Error(`Ka empty bestmove: ${message}`));
          return;
        }
        finish(null, move);
      }
    });

    ws.addEventListener('error', () => {
      finish(new Error('Ka WebSocket error'));
    });
  });
}
