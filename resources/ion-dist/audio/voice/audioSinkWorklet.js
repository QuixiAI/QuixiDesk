"use strict";
(() => {
  var __defProp = Object.defineProperty;
  var __defNormalProp = (obj, key, value) => key in obj ? __defProp(obj, key, { enumerable: true, configurable: true, writable: true, value }) : obj[key] = value;
  var __publicField = (obj, key, value) => __defNormalProp(obj, typeof key !== "symbol" ? key + "" : key, value);

  // components/voice/audio/ringbuf.ts
  var RD = 0;
  var WR = 1;
  var UNDERRUNS = 2;
  var PLAYED = 3;
  var HEADER_SLOTS = 4;
  var HEADER_BYTES = HEADER_SLOTS * Uint32Array.BYTES_PER_ELEMENT;
  var RingBuffer = class {
    constructor(sab) {
      __publicField(this, "capacity");
      __publicField(this, "storage");
      __publicField(this, "indices");
      this.indices = new Uint32Array(sab, 0, HEADER_SLOTS);
      this.storage = new Float32Array(sab, HEADER_BYTES);
      this.capacity = this.storage.length;
    }
    /** Allocate a backing buffer sized for `capacity` Float32 samples + header. */
    static allocate(capacity) {
      const bytes = HEADER_BYTES + capacity * Float32Array.BYTES_PER_ELEMENT;
      const Ctor = typeof SharedArrayBuffer !== "undefined" ? SharedArrayBuffer : ArrayBuffer;
      return new Ctor(bytes);
    }
    /** Producer-side. Writes up to input.length samples; returns count written. */
    push(input) {
      const wr = Atomics.load(this.indices, WR);
      const rd = Atomics.load(this.indices, RD);
      const n = Math.min(input.length, this._availableWrite(rd, wr));
      if (n === 0) {
        return 0;
      }
      const tail = this.capacity - wr;
      if (n <= tail) {
        this.storage.set(input.subarray(0, n), wr);
      } else {
        this.storage.set(input.subarray(0, tail), wr);
        this.storage.set(input.subarray(tail, n), 0);
      }
      Atomics.store(this.indices, WR, (wr + n) % this.capacity);
      return n;
    }
    /** Consumer-side. Reads up to output.length samples; returns count read. */
    pop(output) {
      const rd = Atomics.load(this.indices, RD);
      const wr = Atomics.load(this.indices, WR);
      const n = Math.min(output.length, this._availableRead(rd, wr));
      if (n === 0) {
        return 0;
      }
      const tail = this.capacity - rd;
      if (n <= tail) {
        output.set(this.storage.subarray(rd, rd + n));
      } else {
        output.set(this.storage.subarray(rd, this.capacity));
        output.set(this.storage.subarray(0, n - tail), tail);
      }
      Atomics.store(this.indices, RD, (rd + n) % this.capacity);
      return n;
    }
    /** Reset both heads. Only safe when neither side is concurrently active. */
    clear() {
      Atomics.store(this.indices, RD, 0);
      Atomics.store(this.indices, WR, 0);
    }
    /** Consumer-side discard: fast-forward rd to wr. Safe while producer pushes. */
    drainAll() {
      const wr = Atomics.load(this.indices, WR);
      Atomics.store(this.indices, RD, wr);
    }
    availableRead() {
      const rd = Atomics.load(this.indices, RD);
      const wr = Atomics.load(this.indices, WR);
      return this._availableRead(rd, wr);
    }
    availableWrite() {
      const rd = Atomics.load(this.indices, RD);
      const wr = Atomics.load(this.indices, WR);
      return this._availableWrite(rd, wr);
    }
    isEmpty() {
      return this.availableRead() === 0;
    }
    isFull() {
      return this.availableWrite() === 0;
    }
    /** capacity - 1 (one slot reserved to distinguish full/empty). */
    usableCapacity() {
      return this.capacity - 1;
    }
    incrementUnderrun() {
      Atomics.add(this.indices, UNDERRUNS, 1);
    }
    underrunCount() {
      return Atomics.load(this.indices, UNDERRUNS);
    }
    resetUnderruns() {
      Atomics.store(this.indices, UNDERRUNS, 0);
    }
    /** Consumer increments after writing samples to the destination. */
    addPlayedSamples(n) {
      Atomics.add(this.indices, PLAYED, n);
    }
    playedSamples() {
      return Atomics.load(this.indices, PLAYED);
    }
    resetPlayed() {
      Atomics.store(this.indices, PLAYED, 0);
    }
    _availableRead(rd, wr) {
      return (wr - rd + this.capacity) % this.capacity;
    }
    _availableWrite(rd, wr) {
      return this.capacity - 1 - this._availableRead(rd, wr);
    }
  };

  // components/voice/audio/audioSinkWorklet.ts
  var AUDIO_SINK_PROCESSOR_NAME = "audio-sink-processor";
  var AudioSinkProcessor = class extends AudioWorkletProcessor {
    constructor(opts) {
      super();
      __publicField(this, "ring");
      // Gate underrun counting: process() runs at ~375/sec from the moment the
      // worklet connects, long before any TTS audio arrives. Counting those
      // empty-ring quanta as underruns inflates the dev-tools badge by hundreds
      // before playback even starts. Only count after the first non-empty read.
      __publicField(this, "hasStarted", false);
      // One-shot drain notification latch: prevents flooding the port at ~375/sec.
      // Set by notifyDrained(); cleared in the else-branch when new audio plays,
      // on flush, and on turn_end (when ring is already empty).
      __publicField(this, "drainPosted", false);
      // Underrun tracking arm: set when new-turn audio has actually played (either
      // via the normal else-branch or a full-quantum straddle). Separates the
      // dry-gap guard from drainPosted so the two concerns don't interfere -
      // a straddle latches drainPosted to prevent duplicate notifications, but
      // must still arm gapReady so a subsequent empty quantum counts as a stall.
      __publicField(this, "gapReady", false);
      // Post-playback dry gap awaiting resolution: counted as an underrun only if
      // audio later resumes (a real mid-utterance stall), else discarded on
      // turn_end/flush so the natural end-of-turn drain isn't counted.
      __publicField(this, "dryGapPending", false);
      // Race guard: set when turn_end arrives while the ring still has samples.
      // Prevents residual post-turn drain from re-entering new-turn underrun
      // tracking and causing a false underrun on the next turn's first audio.
      __publicField(this, "postTurnEnd", false);
      // Samples left in the ring when turn_end arrived. postTurnEnd stays set until
      // this many samples have drained. Counting them down (rather than watching
      // for an empty ring) clears the gate deterministically even when the producer
      // pushes next-turn audio into the ring mid-drain - an empty-ring check never
      // fires in that race and would latch the gate across the next turn.
      __publicField(this, "residualSamples", 0);
      this.ring = new RingBuffer(opts.processorOptions.sab);
      this.port.onmessage = (ev) => {
        if (ev.data.type === "flush") {
          this.ring.drainAll();
          this.hasStarted = false;
          this.drainPosted = false;
          this.gapReady = false;
          this.dryGapPending = false;
          this.postTurnEnd = false;
          this.residualSamples = 0;
        } else if (ev.data.type === "turn_end") {
          this.hasStarted = false;
          this.gapReady = false;
          this.dryGapPending = false;
          this.residualSamples = this.ring.availableRead();
          this.postTurnEnd = this.residualSamples > 0;
          if (this.residualSamples === 0) {
            this.notifyDrained();
          }
        }
      };
    }
    process(_inputs, outputs, _params) {
      const channel = outputs[0]?.[0];
      if (!channel) {
        return true;
      }
      const read = this.ring.pop(channel);
      if (read > 0) {
        if (this.postTurnEnd) {
          this.ring.addPlayedSamples(read);
          this.residualSamples -= read;
          if (this.residualSamples <= 0) {
            this.postTurnEnd = false;
            if (this.residualSamples < 0) {
              this.hasStarted = true;
              this.notifyDrained();
              if (read === channel.length) {
                this.gapReady = true;
              }
            } else {
              this.notifyDrained();
            }
          }
        } else {
          this.hasStarted = true;
          this.drainPosted = false;
          this.gapReady = true;
          this.ring.addPlayedSamples(read);
          if (this.dryGapPending) {
            this.ring.incrementUnderrun();
            this.dryGapPending = false;
          }
        }
      }
      if (read < channel.length) {
        channel.fill(0, read);
        if (this.hasStarted && this.gapReady) {
          this.dryGapPending = true;
          if (read === 0 && !this.drainPosted) {
            this.notifyDrained();
          }
        } else if (this.postTurnEnd && read === 0) {
          this.postTurnEnd = false;
          this.notifyDrained();
        }
      }
      return true;
    }
    /**
     * Post the one-shot "drained" notification to the main thread. AudioPlayer
     * routes it to checkPlaybackComplete() - the worklet sink's only completion
     * signal. Latched by drainPosted; reset on new data, flush, and turn_end.
     */
    notifyDrained() {
      if (this.drainPosted) {
        return;
      }
      this.drainPosted = true;
      this.port.postMessage({ type: "drained" });
    }
  };
  registerProcessor(AUDIO_SINK_PROCESSOR_NAME, AudioSinkProcessor);
})();
