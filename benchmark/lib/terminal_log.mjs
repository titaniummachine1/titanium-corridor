/**
 * Immediate terminal output — Node buffers console.log on Windows until newline + flush.
 */

export function termLine(text = '') {
  process.stdout.write(`${text}\n`);
}

export function termThinking({ ply, side, engine }) {
  const who = side === 0 || side === 'White' || side === 'white' ? 'White' : 'Black';
  termLine(`  >> ply ${ply} ${who} · ${engine} thinking…`);
}
