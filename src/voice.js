// A small Web Audio synth for the functional-cable diagram. It is not kabl's engine.
// Two voices so the morph is audible: pulses credited to pattern A play a low, warm
// saw arpeggio; pulses credited to pattern B play a brighter square voice an octave up.
// Each voice's level (its VCA, and a little of its filter) follows the cable.
const ARP_A = [110, 165, 220, 165, 131, 196, 262, 196, 98, 147, 196, 147, 110, 165, 220, 247]; // A minor, low
const ARP_B = [440, 659, 523, 784, 440, 659, 587, 880, 392, 587, 494, 784, 440, 659, 523, 988]; // brighter, higher

function makeVoice(ctx, out, { type, detune, filterType, base, span, q, gain, sub }) {
  const vca = ctx.createGain();
  vca.gain.value = 0;
  const filter = ctx.createBiquadFilter();
  filter.type = filterType;
  filter.frequency.value = base;
  filter.Q.value = q;
  const trim = ctx.createGain();
  trim.gain.value = gain;
  filter.connect(vca).connect(trim).connect(out);
  const oscs = detune.map((cents) => {
    const o = ctx.createOscillator();
    o.type = type;
    o.detune.value = cents;
    o.connect(filter);
    o.start();
    return o;
  });
  let subOsc = null;
  if (sub) {
    subOsc = ctx.createOscillator();
    subOsc.type = "sine";
    const g = ctx.createGain();
    g.gain.value = 0.5;
    subOsc.connect(g).connect(vca);
    subOsc.start();
  }
  return {
    set(t, f, level, ramp) {
      oscs.forEach((o) => o.frequency.setValueAtTime(f, t));
      subOsc?.frequency.setValueAtTime(f / 2, t);
      for (const [param, value] of [[vca.gain, level], [filter.frequency, base + span * level]]) {
        param.cancelScheduledValues(t);
        param.setValueAtTime(param.value, t);
        param.linearRampToValueAtTime(value, t + ramp);
      }
    },
    silence(t) {
      vca.gain.cancelScheduledValues(t);
      vca.gain.setTargetAtTime(0, t, 0.02);
    },
  };
}

export class Voice {
  constructor() {
    this.ctx = null;
  }

  // Must be called from a user gesture (browsers block audio otherwise).
  start() {
    if (!this.ctx) {
      const ctx = (this.ctx = new (window.AudioContext || window.webkitAudioContext)());
      const out = ctx.createGain();
      out.gain.value = 0.16;
      out.connect(ctx.createDynamicsCompressor()).connect(ctx.destination);
      this.a = makeVoice(ctx, out, { type: "sawtooth", detune: [-7, 7], filterType: "lowpass", base: 500, span: 2600, q: 6, gain: 1, sub: true });
      this.b = makeVoice(ctx, out, { type: "square", detune: [0, 12], filterType: "bandpass", base: 1200, span: 1800, q: 2.5, gain: 0.55, sub: false });
    }
    return this.ctx.resume();
  }

  get on() {
    return this.ctx?.state === "running";
  }

  // One clock pulse: each voice moves to its next note, and its level ramps to
  // `la` / `lb` (the share of this pulse credited to pattern A / B) over `glideMs`.
  step(i, la, lb, glideMs) {
    if (!this.on) return;
    const t = this.ctx.currentTime, ramp = Math.max(0.006, glideMs / 1000);
    this.a.set(t, ARP_A[i % ARP_A.length], la, ramp);
    this.b.set(t, ARP_B[i % ARP_B.length], lb, ramp);
  }

  silence() {
    if (!this.ctx) return;
    const t = this.ctx.currentTime;
    this.a.silence(t);
    this.b.silence(t);
  }

  stop() {
    this.silence();
    return this.ctx?.suspend();
  }
}
