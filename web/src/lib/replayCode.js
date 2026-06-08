/**
 * Copy-paste replay format for terminal ↔ web.
 *
 *   tq1 e2 e8 e3 e7 d6h ...
 *   tq1#{"game":1,"winner":"Ka"} e2 e8 ...
 */

import { parseAlgebraic, toAlgebraic } from './gameLogic.js';

const PREFIX = 'tq1';

export function encodeReplayFromAlgebraic(algebraicMoves, meta = null) {
  const body = algebraicMoves.join(' ');
  if (!meta || Object.keys(meta).length === 0) {
    return `${PREFIX} ${body}`;
  }
  return `${PREFIX}#${JSON.stringify(meta)} ${body}`;
}

export function encodeReplayFromActions(actions, meta = null) {
  return encodeReplayFromAlgebraic(actions.map((a) => toAlgebraic(a)), meta);
}

export function decodeReplayCode(text) {
  const trimmed = text.trim();
  if (!trimmed) {
    throw new Error('Empty replay');
  }

  let meta = null;
  let movesPart = trimmed;

  if (trimmed.startsWith(PREFIX)) {
    const hashIdx = trimmed.indexOf('#');
    const spaceAfterPrefix = trimmed.indexOf(' ');
    if (hashIdx > 0 && spaceAfterPrefix > hashIdx) {
      meta = JSON.parse(trimmed.slice(hashIdx + 1, spaceAfterPrefix));
      movesPart = trimmed.slice(spaceAfterPrefix + 1);
    } else if (spaceAfterPrefix > 0) {
      movesPart = trimmed.slice(spaceAfterPrefix + 1);
    } else {
      movesPart = '';
    }
  } else if (/^REPLAY/i.test(trimmed)) {
    const lines = trimmed.split(/\r?\n/).map((l) => l.trim()).filter(Boolean);
    const codeLine = lines.find((l) => l.startsWith(PREFIX)) ?? lines[lines.length - 1];
    return decodeReplayCode(codeLine);
  }

  const matches = movesPart.match(/\b[a-i][1-9][hv]?\b/gi) ?? [];
  const tokens = matches.map((token) => token.toLowerCase());
  if (tokens.length === 0) {
    throw new Error('No moves in replay');
  }

  const actions = tokens.map((token) => parseAlgebraic(token));
  return { actions, meta, algebraic: tokens };
}

export function formatReplayBlock(code, { label = 'REPLAY — paste in web → Replay tab' } = {}) {
  return [
    '',
    `┌─ ${label} ─────────────────────────────────────────`,
    code,
    '└────────────────────────────────────────────────────',
    '',
  ].join('\n');
}
