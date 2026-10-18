// Plays the four rendered stems through Web Audio.
//
// All stems go into one 4-channel buffer so they stay sample-locked, then a
// splitter feeds one gain per channel. Muting sets a gain to 0, so it takes
// effect immediately, like the CLI's `play`.

export const CHANNELS = ['pulse1', 'pulse2', 'wave', 'noise'];
// The same master scale as apu::mix.
const MASTER_GAIN = 0.6;

export class Player {
  ctx = null;
  source = null;
  gains = [];
  muted = CHANNELS.map(() => false);
  repeat = false;
  startedAt = 0;
  duration = 0;

  get playing() {
    return this.source !== null;
  }

  /** Seconds into the song, or 0 when stopped. */
  get position() {
    if (!this.source) return 0;
    const elapsed = this.ctx.currentTime - this.startedAt;
    return this.duration > 0 ? elapsed % this.duration : 0;
  }

  /**
   * Start `stems` (planar Float32Array, CHANNELS.length equal parts) at
   * `offset` seconds. Replaces whatever is playing.
   */
  async play(stems, sampleRate, offset, onEnded) {
    this.ctx ??= new AudioContext();
    await this.ctx.resume();
    this.stop();

    const ctx = this.ctx;
    const len = stems.length / CHANNELS.length;
    const buffer = ctx.createBuffer(CHANNELS.length, len, sampleRate);
    CHANNELS.forEach((_, ch) => {
      buffer.copyToChannel(stems.subarray(ch * len, (ch + 1) * len), ch);
    });

    const master = ctx.createGain();
    master.gain.value = MASTER_GAIN;
    master.connect(ctx.destination);
    const splitter = ctx.createChannelSplitter(CHANNELS.length);
    this.gains = CHANNELS.map((_, ch) => {
      const gain = ctx.createGain();
      gain.gain.value = this.muted[ch] ? 0 : 1;
      splitter.connect(gain, ch);
      gain.connect(master);
      return gain;
    });

    const source = ctx.createBufferSource();
    source.buffer = buffer;
    source.loop = this.repeat;
    source.connect(splitter);
    source.onended = () => {
      // A replaced source also fires onended; only the current one counts.
      if (this.source === source) {
        this.source = null;
        onEnded?.();
      }
    };

    const start = offset < buffer.duration ? offset : 0;
    this.duration = buffer.duration;
    this.startedAt = ctx.currentTime - start;
    this.source = source;
    source.start(0, start);
  }

  stop() {
    const source = this.source;
    this.source = null;
    source?.stop();
  }

  setMuted(ch, muted) {
    this.muted[ch] = muted;
    if (this.gains[ch]) this.gains[ch].gain.value = muted ? 0 : 1;
  }

  setRepeat(repeat) {
    this.repeat = repeat;
    if (this.source) this.source.loop = repeat;
  }
}
