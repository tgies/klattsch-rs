// AudioWorklet shim around klattsch-wasm. See ../README.md for usage.
//
// Message protocol (node.port.postMessage):
//   { type: 'compile', text: '...' }                    recompile + queue
//   { type: 'frame', target: {...}, transitionMs: ms }  live parameter ramp
//   { type: 'schedule', schedule: [{atMs,target,transitionMs}, ...] }
//   { type: 'reset' }                                    clear state
//
// processorOptions:
//   wasmModule  (WebAssembly.Module, required)
//   text        (initial phoneme string, optional)

import { initSync, FormantSynth, compileStringAt } from './klattsch_wasm.js';

let initialized = false;

class KlattschFormantProcessor extends AudioWorkletProcessor {
  constructor({ processorOptions = {} } = {}) {
    super();

    if (!initialized) {
      if (!processorOptions.wasmModule) {
        throw new Error(
          'klattsch worklet: processorOptions.wasmModule (WebAssembly.Module) is required'
        );
      }
      initSync({ module: processorOptions.wasmModule });
      initialized = true;
    }

    this.synth = new FormantSynth(sampleRate);

    if (processorOptions.text) {
      const result = compileStringAt(processorOptions.text, sampleRate);
      const sched = result.takeSchedule();
      if (sched) this.synth.queueSchedule(sched);
    }

    this.port.onmessage = (e) => {
      const msg = e.data;
      if (!msg || typeof msg !== 'object') return;
      switch (msg.type) {
        case 'compile':
          if (typeof msg.text === 'string') {
            const result = compileStringAt(msg.text, sampleRate);
            const sched = result.takeSchedule();
            if (sched) this.synth.queueSchedule(sched);
          }
          break;
        case 'frame':
          this.synth.setTarget(msg.target ?? {}, msg.transitionMs ?? 30);
          break;
        case 'schedule':
          this.synth.queueScheduleFromMs(msg.schedule ?? []);
          break;
        case 'reset':
          this.synth.reset();
          break;
      }
    };
  }

  process(_inputs, outputs) {
    const out = outputs[0][0];
    this.synth.process(out);
    if (outputs[0].length > 1) outputs[0][1].set(out);
    return true;
  }
}

registerProcessor('klattsch-formant-processor', KlattschFormantProcessor);
