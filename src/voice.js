// A small Web Audio voice for the functional-cable diagram. It is not kabl's engine:
// the cable's level drives a VCA (and opens a filter a little) on a plain saw tone
// playing a fixed arpeggio, so you hear what pattern, chance, morph and glide do.
const ARP = [110, 165, 220, 165, 131, 196, 262, 196, 98, 147, 196, 147, 110, 165, 220, 247]; // A minor

export class Voice {
  constructor() {
    this.ctx = null;
  }

  // Must be called from a user gesture (browsers block audio otherwise).
  start() {
    if (!this.ctx) {
      const ctx = (this.ctx = new (window.AudioContext || window.webkitAudioContext)());
      this.out = ctx.createGain();
      this.out.gain.value = 0.16;
      const comp = ctx.createDynamicsCompressor();
      this.out.connect(comp).connect(ctx.destination);
      this.vca = ctx.createGain();
      this.vca.gain.value = 0;
      this.filter = ctx.createBiquadFilter();
      this.filter.type = "lowpass";
      this.filter.frequency.value = 700;
      this.filter.Q.value = 6;
      this.filter.connect(this.vca).connect(this.out);
      this.oscs = [-7, 7].map((cents) => {
        const o = ctx.createOscillator();
        o.type = "sawtooth";
        o.detune.value = cents;
        o.frequency.value = ARP[0];
        o.connect(this.filter);
        o.start();
        return o;
      });
      const sub = ctx.createOscillator();
      sub.type = "sine";
      sub.frequency.value = ARP[0] / 2;
      const subGain = ctx.createGain();
      subGain.gain.value = 0.5;
      sub.connect(subGain).connect(this.vca);
      sub.start();
      this.sub = sub;
    }
    return this.ctx.resume();
  }

  get on() {
    return this.ctx?.state === "running";
  }

  // One clock pulse: new pitch, and the cable's level ramps to `level` over `glideMs`.
  step(i, level, glideMs) {
    if (!this.on) return;
    const t = this.ctx.currentTime, ramp = Math.max(0.006, glideMs / 1000);
    const f = ARP[i % ARP.length];
    this.oscs.forEach((o) => o.frequency.setValueAtTime(f, t));
    this.sub.frequency.setValueAtTime(f / 2, t);
    for (const [param, value] of [[this.vca.gain, level], [this.filter.frequency, 500 + 2600 * level]]) {
      param.cancelScheduledValues(t);
      param.setValueAtTime(param.value, t);
      param.linearRampToValueAtTime(value, t + ramp);
    }
  }

  silence() {
    if (!this.ctx) return;
    const t = this.ctx.currentTime;
    this.vca.gain.cancelScheduledValues(t);
    this.vca.gain.setTargetAtTime(0, t, 0.02);
  }

  stop() {
    this.silence();
    return this.ctx?.suspend();
  }
}
