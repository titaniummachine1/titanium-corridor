import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const v7Html = 'C:/Users/Terminatort8000/Downloads/quoridor (6).html';
const v8Html = 'C:/Users/Terminatort8000/Downloads/quoridor (5).html';

function extractEngine(htmlPath) {
  const html = fs.readFileSync(htmlPath, 'utf8');
  const m = html.match(/<script id="enginecode">([\s\S]*?)<\/script>/);
  if (!m) throw new Error(`no enginecode in ${htmlPath}`);
  return { html, engine: m[1].trim() };
}

const v7 = extractEngine(v7Html);
const v8 = extractEngine(v8Html);

fs.writeFileSync(path.join(root, '_vendor/acev7_engine.js'), v7.engine);
fs.writeFileSync(path.join(root, '_vendor/acev8_engine.js'), v8.engine);

console.log('=== Full HTML ===');
console.log('v7 (6).html:', v7.html.length, `(${(v7.html.length / 1024).toFixed(1)} KB)`);
console.log('v8 (5).html:', v8.html.length, `(${(v8.html.length / 1024).toFixed(1)} KB)`);
console.log('delta:', v8.html.length - v7.html.length, `(${(Math.abs(v8.html.length - v7.html.length) / 1024).toFixed(1)} KB)`);

console.log('\n=== Engine script only ===');
console.log('v7 engine:', v7.engine.length, `(${(v7.engine.length / 1024).toFixed(1)} KB)`);
console.log('v8 engine:', v8.engine.length, `(${(v8.engine.length / 1024).toFixed(1)} KB)`);
console.log('delta:', v8.engine.length - v7.engine.length, `(${(Math.abs(v8.engine.length - v7.engine.length) / 1024).toFixed(1)} KB)`);

const coach7 = v7.html.match(/<script id="coachcode">([\s\S]*?)<\/script>/)?.[1]?.length ?? 0;
const coach8 = v8.html.match(/<script id="coachcode">([\s\S]*?)<\/script>/)?.[1]?.length ?? 0;
console.log('\n=== Coach script ===');
console.log('v7 coach:', coach7, `(${(coach7 / 1024).toFixed(1)} KB)`);
console.log('v8 coach:', coach8, `(${(coach8 / 1024).toFixed(1)} KB)`);

function stripLogic(s) {
  return s
    .replace(/^\/\*[\s\S]*?\*\/\s*/, '')
    .replace(/"use strict";\s*/, '')
    .replace(/var NET_DATA = \{[\s\S]*?\};/g, 'NET;')
    .replace(/\s+/g, ' ')
    .trim();
}

const nets7 = [...v7.engine.matchAll(/var NET_DATA = (\{[\s\S]*?\});/g)];
const nets8 = [...v8.engine.matchAll(/var NET_DATA = (\{[\s\S]*?\});/g)];
console.log('\n=== NET_DATA ===');
for (let i = 0; i < Math.max(nets7.length, nets8.length); i++) {
  const a = nets7[i];
  const b = nets8[i];
  if (!a || !b) {
    console.log(`block ${i}: count mismatch v7=${nets7.length} v8=${nets8.length}`);
    continue;
  }
  const pa = JSON.parse(a[1]);
  const pb = JSON.parse(b[1]);
  const same = JSON.stringify(pa) === JSON.stringify(pb);
  console.log(`block ${i}: H=${pa.H} v7=${a[0].length}b v8=${b[0].length}b identical=${same}`);
}

const logic7 = stripLogic(v7.engine);
const logic8 = stripLogic(v8.engine);
console.log('\n=== Logic (no NET blobs) ===');
console.log('v7:', logic7.length, 'chars');
console.log('v8:', logic8.length, 'chars');
console.log('identical:', logic7 === logic8);

// Method-level diff
function extractProtos(s, prefix) {
  const re = new RegExp(`${prefix}\\.prototype\\.(\\w+) = function \\(([^)]*)\\) \\{([\\s\\S]*?)\\n\\};`, 'g');
  const out = {};
  let m;
  while ((m = re.exec(s))) out[m[1]] = m[3].replace(/\s+/g, ' ').trim();
  return out;
}

for (const [label, prefix] of [
  ['Search', 'Search'],
  ['Quoridor', 'Quoridor'],
]) {
  const a = extractProtos(v7.engine, prefix);
  const b = extractProtos(v8.engine, prefix);
  const names = [...new Set([...Object.keys(a), ...Object.keys(b)])].sort();
  console.log(`\n=== ${label} methods ===`);
  for (const n of names) {
    if (!a[n] || !b[n]) console.log(`  ${n}: missing v7=${!!a[n]} v8=${!!b[n]}`);
    else if (a[n] !== b[n]) console.log(`  ${n}: DIFF (${a[n].length} vs ${b[n].length})`);
    else console.log(`  ${n}: same`);
  }
}

// Search parity at fixed depth
const { writeFileSync, mkdtempSync } = await import('node:fs');
const { tmpdir } = await import('node:os');
const { join } = await import('node:path');
const { execSync } = await import('node:child_process');
const tmp = mkdtempSync(join(tmpdir(), 'ace-'));
writeFileSync(join(tmp, 'run.mjs'), `
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const v7 = require('${path.join(root, '_vendor/acev7_engine.js').replace(/\\/g, '/')}');
const v8 = require('${path.join(root, '_vendor/acev8_engine.js').replace(/\\/g, '/')}');
for (const d of [6,8,10,12]) {
  const r7 = new v7.Search(new v7.Quoridor()).think(1e9,d,true);
  const r8 = new v8.Search(new v8.Quoridor()).think(1e9,d,true);
  const ok = r7.move===r8.move && r7.score===r8.score && r7.nodes===r8.nodes;
  console.log('d'+d, ok?'SAME':'DIFF', JSON.stringify({v7:r7,v8:r8}));
}
`);
execSync('node run.mjs', { cwd: tmp, stdio: 'inherit' });
