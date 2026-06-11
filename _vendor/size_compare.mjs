import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));

const v7 = fs.readFileSync(path.join(root, '_vendor/acev7_engine.js'), 'utf8');
const v8 = fs.readFileSync(path.join(root, '_vendor/acev8_engine.js'), 'utf8');

console.log('=== Raw engine JS ===');
console.log(`v7: ${v7.length} bytes (${(v7.length / 1024).toFixed(1)} KB)`);
console.log(`v8: ${v8.length} bytes (${(v8.length / 1024).toFixed(1)} KB)`);
console.log(`delta v8-v7: ${v8.length - v7.length} bytes`);

function stripLogic(s) {
  return s
    .replace(/^\/\*[\s\S]*?\*\/\s*/, '')
    .replace(/"use strict";\s*/, '')
    .replace(/var NET_DATA = \{[\s\S]*?\};/g, 'NET;')
    .replace(/\s+/g, ' ')
    .trim();
}

const nets7 = [...v7.matchAll(/var NET_DATA = (\{[\s\S]*?\});/g)];
const nets8 = [...v8.matchAll(/var NET_DATA = (\{[\s\S]*?\});/g)];
console.log('\n=== NET_DATA blocks ===');
console.log(`v7 blocks: ${nets7.length}, total ${nets7.reduce((a, m) => a + m[0].length, 0)} chars`);
console.log(`v8 blocks: ${nets8.length}, total ${nets8.reduce((a, m) => a + m[0].length, 0)} chars`);
for (let i = 0; i < Math.max(nets7.length, nets8.length); i++) {
  const a = nets7[i]?.[1];
  const b = nets8[i]?.[1];
  if (!a || !b) {
    console.log(`  block ${i}: missing`);
    continue;
  }
  const pa = JSON.parse(a);
  const pb = JSON.parse(b);
  console.log(`  block ${i} H=${pa.H}: identical=${JSON.stringify(pa) === JSON.stringify(pb)}`);
}

const logic7 = stripLogic(v7);
const logic8 = stripLogic(v8);
console.log('\n=== Logic (no NET, no header) ===');
console.log(`v7 logic: ${logic7.length} chars`);
console.log(`v8 logic: ${logic8.length} chars`);
console.log(`identical: ${logic7 === logic8}`);

if (logic7 !== logic8) {
  let i = 0;
  while (i < logic7.length && i < logic8.length && logic7[i] === logic8[i]) i++;
  console.log(`first logic diff at ${i}`);
  console.log('v7:', logic7.slice(i, i + 200));
  console.log('v8:', logic8.slice(i, i + 200));
}

// HTML vs engine
for (const name of ['quoridor (4).html', 'quoridor (5).html']) {
  const p = `C:/Users/Terminatort8000/Downloads/${name}`;
  if (!fs.existsSync(p)) continue;
  const html = fs.readFileSync(p, 'utf8');
  const m = html.match(/<script id="enginecode">([\s\S]*?)<\/script>/);
  const engine = m?.[1]?.trim() ?? '';
  const coach = html.match(/<script id="coachcode">([\s\S]*?)<\/script>/)?.[1]?.length ?? 0;
  const ui = html.length - engine.length - coach;
  console.log(`\n=== ${name} ===`);
  console.log(`full HTML: ${html.length} (${(html.length / 1024).toFixed(1)} KB)`);
  console.log(`engine script: ${engine.length} (${(engine.length / 1024).toFixed(1)} KB)`);
  console.log(`coach script: ${coach} (${(coach / 1024).toFixed(1)} KB)`);
  console.log(`rest (UI/css/app): ~${ui} (${(ui / 1024).toFixed(1)} KB)`);
  console.log(`engine === v8 file: ${engine === v8}`);
}
