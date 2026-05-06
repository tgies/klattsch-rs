#!/usr/bin/env node
// Render the parity-test reference WAVs from the JavaScript klattsch engine.
// Run from the workspace root:
//
//   npm run goldens
//
// Writes one .wav per entry in INPUTS to ../tests/golden/<name>.wav.
// Re-running this regenerates the goldens; treat that as a deliberate decision
// (it means accepting a behavior change in the JS reference).

import { writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

import { compileString, FormantSynth, encodeWav } from 'klattsch';

const SR = 48000;

// Curated parity-test corpus. Names become file basenames; keep them stable.
const INPUTS = [
  ['hello',         'HH AH L OW'],
  ['hello-world',   'HH EH L OW W ER L D'],
  ['single-vowel-a4', 'b220 AH'],
  ['plosives',      'P AH T AH K AH'],
  ['diphthongs',    'AY EY OY'],
  ['fricatives',    'S SH F TH'],
  ['nasals',        'M N NG'],
  ['syllables',     '( HH AH ) ( L OW )'],
  ['stress',        "AH ! AH"],
  ['steady-tone',   'b120 AH AH AH AH'],
];

const here = dirname(fileURLToPath(import.meta.url));
const outDir = resolve(here, '..', 'tests', 'golden');
mkdirSync(outDir, { recursive: true });

for (const [name, text] of INPUTS) {
  const { schedule, totalMs, warnings } = compileString(text);
  if (warnings.length) {
    console.error(`[${name}] warnings: ${warnings.join(', ')}`);
  }
  const totalSamples = Math.max(1, Math.ceil(totalMs * SR / 1000));
  const buf = new Float32Array(totalSamples);
  const synth = new FormantSynth({ sampleRate: SR, schedule });
  synth.process(buf);

  // Skip peak normalization so the goldens compare byte-aligned with the
  // raw Rust output (the parity test renders Rust without normalization too).
  const { bytes } = encodeWav(buf, SR, { peakNormalize: 0 });
  const path = resolve(outDir, `${name}.wav`);
  writeFileSync(path, bytes);
  console.log(`wrote ${path} (${(bytes.length / 1024).toFixed(0)} KB, ${(totalMs/1000).toFixed(2)}s)`);
}

// Also emit a manifest with the inputs so the Rust parity test stays in sync
// without anyone having to remember to update both files.
const manifest = INPUTS.map(([name, text]) => ({ name, text, sampleRate: SR }));
writeFileSync(resolve(outDir, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
console.log(`wrote ${outDir}/manifest.json`);
