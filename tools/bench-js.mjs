#!/usr/bin/env node
// Wall-clock benchmark of the JavaScript klattsch engine.
//
//   npm run bench:js -- <input.txt> [iterations]

import { readFileSync } from 'node:fs';
import { compileString, FormantSynth } from 'klattsch';

const path = process.argv[2];
if (!path) {
  console.error('usage: npm run bench:js -- <input.txt> [iterations]');
  process.exit(1);
}
const iters = parseInt(process.argv[3] ?? '100', 10);
const text = readFileSync(path, 'utf8');
const bytes = Buffer.byteLength(text, 'utf8');
const SR = 48000;

// Warmup so V8 JITs the hot paths before we measure.
for (let i = 0; i < 5; i++) {
  const r = compileString(text);
  const total = Math.max(1, Math.ceil(r.totalMs * SR / 1000));
  const buf = new Float32Array(total);
  const s = new FormantSynth({ sampleRate: SR, schedule: r.schedule });
  s.process(buf);
}

const compileNs = new Float64Array(iters);
const renderNs = new Float64Array(iters);
let totalSamples = 0;
let totalMs = 0;

for (let i = 0; i < iters; i++) {
  const t0 = process.hrtime.bigint();
  const r = compileString(text);
  const t1 = process.hrtime.bigint();
  const total = Math.max(1, Math.ceil(r.totalMs * SR / 1000));
  const buf = new Float32Array(total);
  const s = new FormantSynth({ sampleRate: SR, schedule: r.schedule });
  s.process(buf);
  const t2 = process.hrtime.bigint();
  compileNs[i] = Number(t1 - t0);
  renderNs[i] = Number(t2 - t1);
  totalSamples = buf.length;
  totalMs = r.totalMs;
}

const stats = (arr) => {
  const sorted = Array.from(arr).sort((a, b) => a - b);
  const sum = sorted.reduce((a, b) => a + b, 0);
  const mean = sum / sorted.length;
  const median = sorted[Math.floor(sorted.length / 2)];
  const p99 = sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.99))];
  const min = sorted[0];
  return { mean, median, min, p99 };
};
const c = stats(compileNs);
const r = stats(renderNs);
const totals = new Float64Array(iters);
for (let i = 0; i < iters; i++) totals[i] = compileNs[i] + renderNs[i];
const t = stats(totals);

const audioSeconds = totalMs / 1000;
const totalMeanSeconds = t.mean / 1e9;
const realtimeFactor = audioSeconds / totalMeanSeconds;

const fmt = (ns) => (ns / 1e3).toFixed(1).padStart(10);
console.log(`input              : ${path} (${bytes} bytes)`);
console.log(`iterations         : ${iters}`);
console.log(`output samples     : ${totalSamples} (${audioSeconds.toFixed(2)}s of audio at ${SR} Hz)`);
console.log();
console.log('                          mean        median       min          p99');
console.log(`compile (us)        : ${fmt(c.mean)}  ${fmt(c.median)}  ${fmt(c.min)}  ${fmt(c.p99)}`);
console.log(`render  (us)        : ${fmt(r.mean)}  ${fmt(r.median)}  ${fmt(r.min)}  ${fmt(r.p99)}`);
console.log(`total   (us)        : ${fmt(t.mean)}  ${fmt(t.median)}  ${fmt(t.min)}  ${fmt(t.p99)}`);
console.log();
console.log(`realtime factor    : ${realtimeFactor.toFixed(1)}x  (mean ${(t.mean / 1e6).toFixed(2)}ms wall to render ${audioSeconds.toFixed(2)}s of audio)`);
