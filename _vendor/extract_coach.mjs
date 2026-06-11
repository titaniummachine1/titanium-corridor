import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const htmlPath = 'C:/Users/Terminatort8000/Downloads/quoridor (5).html';

if (!fs.existsSync(htmlPath)) {
  console.error('HTML missing:', htmlPath);
  process.exit(1);
}

const html = fs.readFileSync(htmlPath, 'utf8');
const coachMatch = html.match(/<script id="coachcode">([\s\S]*?)<\/script>/);
const appMatch = html.match(/<script id="appcode">([\s\S]*?)<\/script>/);

if (!coachMatch) {
  console.error('no coachcode');
  process.exit(1);
}

const coach = coachMatch[1].trim();
fs.writeFileSync(path.join(root, '_vendor/acev8_coach.js'), coach);
console.log('coach bytes:', coach.length);
console.log('app bytes:', appMatch?.[1]?.length ?? 0);

for (const k of [
  'radar',
  'curriculum',
  'snap',
  'daily',
  'weakness',
  'guess',
  'drill',
  'elo',
  'corpus',
  'tempo',
  'cut',
  'defense',
  'endgame',
  'attack',
]) {
  const n = (coach.match(new RegExp(k, 'gi')) || []).length;
  if (n) console.log(k, n);
}
