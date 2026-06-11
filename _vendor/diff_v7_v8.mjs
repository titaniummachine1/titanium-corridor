import fs from 'node:fs';

const v7 = fs.readFileSync(new URL('./acev7_engine.js', import.meta.url), 'utf8');
const v8 = fs.readFileSync(new URL('./acev8_engine.js', import.meta.url), 'utf8');

function strip(s) {
  return s
    .replace(/^\/\*[\s\S]*?\*\/\s*/, '')
    .replace(/"use strict";\s*/, '')
    .replace(/var NET_DATA = \{[\s\S]*?\};/g, 'NET_DATA;')
    .replace(/\s+/g, ' ')
    .trim();
}

const s7 = strip(v7);
const s8 = strip(v8);
console.log('file bytes:', v7.length, v8.length);
console.log('stripped logic equal:', s7 === s8);

function extractProtos(s, prefix) {
  const re = new RegExp(`${prefix}\\.prototype\\.(\\w+) = function \\(([^)]*)\\) \\{([\\s\\S]*?)\\n\\};`, 'g');
  const out = {};
  let m;
  while ((m = re.exec(s))) {
    out[m[1]] = m[3].replace(/\s+/g, ' ').trim();
  }
  return out;
}

for (const [label, prefix] of [
  ['Search', 'Search'],
  ['Quoridor', 'Quoridor'],
]) {
  const a = extractProtos(v7, prefix);
  const b = extractProtos(v8, prefix);
  const names = [...new Set([...Object.keys(a), ...Object.keys(b)])].sort();
  console.log(`\n${label} methods:`);
  for (const n of names) {
    if (!a[n] || !b[n]) console.log(`  ${n}: missing in ${!a[n] ? 'v7' : 'v8'}`);
    else if (a[n] !== b[n]) console.log(`  ${n}: DIFFERENT (${a[n].length} vs ${b[n].length} chars)`);
    else console.log(`  ${n}: identical`);
  }
}

function extractNet(s) {
  return [...s.matchAll(/var NET_DATA = (\{[\s\S]*?\});/g)].map((m) => JSON.parse(m[1]));
}

const n7 = extractNet(v7);
const n8 = extractNet(v8);
console.log('\nNET_DATA blocks:', n7.length, n8.length);
for (let i = 0; i < Math.max(n7.length, n8.length); i++) {
  const a = n7[i];
  const b = n8[i];
  if (!a || !b) {
    console.log(`  block ${i}: missing`);
    continue;
  }
  const same = JSON.stringify(a) === JSON.stringify(b);
  console.log(`  block ${i}: H=${a.H} identical=${same}`);
  if (!same) {
    for (const key of ['Wskip', 'B1', 'W2']) {
      const eq = JSON.stringify(a[key]) === JSON.stringify(b[key]);
      console.log(`    ${key}: ${eq ? 'same' : 'DIFF'}`);
    }
  }
}
