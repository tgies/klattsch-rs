# klattsch-wasm

WebAssembly build of the klattsch synthesizer.

## Build

```sh
cd crates/klattsch-wasm
wasm-pack build --target web
# Output: crates/klattsch-wasm/pkg/
```

## Offline render

```javascript
import init, { FormantSynth, compileString, encodeWav } from './pkg/klattsch_wasm.js';

await init();

const sr = 48000;
const result = compileString('HH AH L OW');
const synth = new FormantSynth(sr);
synth.queueSchedule(result.takeSchedule());

const buf = new Float32Array(Math.ceil(result.totalMs * sr / 1000));
synth.process(buf);

const wavBytes = encodeWav(buf, sr, 0.95);
```

## AudioWorklet

The worklet uses synchronous wasm init; compile the module on the main thread
and pass it via `processorOptions`.

```javascript
import init from './pkg/klattsch_wasm.js';

const ctx = new AudioContext();
await ctx.audioWorklet.addModule('./js/formant-processor.js');

const wasmModule = await WebAssembly.compileStreaming(
  fetch('./pkg/klattsch_wasm_bg.wasm')
);
const node = new AudioWorkletNode(ctx, 'klattsch-formant-processor', {
  processorOptions: { wasmModule, text: 'HH AH L OW' },
});
node.connect(ctx.destination);
```

The worklet compiles schedules at the live `AudioContext.sampleRate`, so it
works at 44.1 kHz, 48 kHz, etc.

### Live updates via `node.port`

```javascript
// Reset and recompile a new utterance:
node.port.postMessage({ type: 'compile', text: 'HH EH L OW W ER L D' });

// Live parameter ramp (matches the original JS engine's 'frame' message):
node.port.postMessage({
  type: 'frame',
  target: { F0: 220, voicing: 1, A1: 1.0, A2: 0.9, A3: 0.7 },
  transitionMs: 30,
});

// Push a pre-built schedule (matches the original 'schedule' message):
node.port.postMessage({
  type: 'schedule',
  schedule: [
    { atMs: 0,   target: { F0: 120, voicing: 1, A1: 1, A2: 0.9, A3: 0.7 }, transitionMs: 30 },
    { atMs: 200, target: { F0: 220 }, transitionMs: 50 },
  ],
});

node.port.postMessage({ type: 'reset' });
```

`target` field names match the original engine's PARAMS array: `F0`,
`voicing`, `F1`, `BW1`, `A1`, ..., `vibratoDepth`, `vibratoRate`,
`tremoloDepth`, `tremoloRate`, `aspiration`, `tilt`, `effort`, `gain`.

## API

| JS                         | Wasm equivalent                  |
|----------------------------|----------------------------------|
| `compileString(text)`      | `compileString(text)`            |
| `new FormantSynth(opt)`    | `new FormantSynth(sr)`           |
| `synth.process(buf)`       | `synth.process(buf)`             |
| `synth.queueSchedule()`    | `synth.queueSchedule(handle)`    |
| `synth.queueSchedule(arr)` | `synth.queueScheduleFromMs(arr)` |
| `synth.setTarget(t, ms)`   | `synth.setTarget(t, ms)`         |
| `synth.reset()`            | `synth.reset()`                  |
| `encodeWav(buf, sr)`       | `encodeWav(buf, sr, peak)`       |

`compileString` returns a `CompileResult`. Call `.takeSchedule()` to move the
schedule out; read `.totalMs` and `.warnings` for diagnostics.
