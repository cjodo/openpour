// One playback clock for every live visual. Status messages carry the
// machine's own time (`ms`); the app shows the machine as it was a fixed
// delay ago and interpolates between messages, so the numbers, beaker, charts
// and nozzle all glide together instead of each jumping when data arrives.
//
// The clock runs at the rate machine time actually arrives: real time on
// hardware, about a quarter of that in Wokwi, faster with the host
// simulator's --speed. Arrival bursts are smoothed out rather than replayed.

import type { Status } from './types';

/** How far behind the newest data the display runs, in machine ms. A bit
 * more than one status period, so there's always a next frame to glide to. */
const LATENCY_MS = 150;
/** Further behind than this (a stall, a reconnect), jump instead of gliding. */
const JUMP_MS = 2000;
/** Machine time kept for interpolation. */
const KEEP_MS = 5000;

const lerp = (a: number, b: number, k: number) => a + (b - a) * k;

export class Timeline {
  private frames: Status[] = [];
  private play = 0;
  /** Machine ms per real ms, measured from arrivals. */
  private rate = 1;
  private lastTick = 0;
  private lastArrival = 0;

  constructor(private instant: () => boolean) {}

  /** The newest machine time received, or null before any status. */
  get newest(): number | null {
    return this.frames.at(-1)?.ms ?? null;
  }

  get playMs(): number {
    return this.play;
  }

  push(s: Status, now: number) {
    const last = this.frames.at(-1);
    if (!last || s.ms < last.ms - 1000) {
      // First message, or the machine restarted.
      this.frames = [s];
      this.play = s.ms - LATENCY_MS;
      this.rate = 1;
      this.lastArrival = now;
      this.lastTick = now;
      return;
    }
    if (s.ms <= last.ms) return; // duplicate (e.g. requested on connect)
    const dReal = now - this.lastArrival;
    if (dReal > 20 && dReal < JUMP_MS) {
      const inst = (s.ms - last.ms) / dReal;
      this.rate = Math.min(20, Math.max(0.05, this.rate * 0.8 + inst * 0.2));
    }
    this.lastArrival = now;
    this.frames.push(s);
    while (this.frames.length > 2 && this.frames[1].ms < this.play - KEEP_MS) this.frames.shift();
  }

  /** Advances the clock to real time `now`; returns the machine ms to show. */
  advance(now: number): number {
    const newest = this.newest;
    if (newest == null) return 0;
    if (this.instant()) return (this.play = newest);
    const dt = Math.min(now - this.lastTick, 250);
    this.lastTick = now;
    this.play += dt * this.rate;
    const err = newest - LATENCY_MS - this.play;
    if (Math.abs(err) > JUMP_MS) this.play = newest - LATENCY_MS;
    else this.play += err * Math.min(1, dt / 400); // ease drift out over ~0.4 s
    this.play = Math.min(this.play, newest);
    return this.play;
  }

  /** True while the display is still catching up with what has arrived. */
  get settling(): boolean {
    const newest = this.newest;
    return newest != null && newest - this.play > 1;
  }

  /** The machine state at the playback time, continuous values interpolated. */
  view(): Status | null {
    const f = this.frames;
    if (!f.length) return null;
    let i = f.length - 1;
    while (i > 0 && f[i].ms > this.play) i--;
    const a = f[i];
    const b = f[i + 1];
    if (!b || this.play <= a.ms) return a;
    const k = (this.play - a.ms) / (b.ms - a.ms);
    // Values only change smoothly within one state; across a change, hold.
    if (a.state !== b.state) return a;
    const v: Status = { ...a, poured: lerp(a.poured, b.poured, k), flow: lerp(a.flow, b.flow, k) };
    if (a.elapsed != null && b.elapsed != null) v.elapsed = lerp(a.elapsed, b.elapsed, k);
    if (a.waitLeft != null && b.waitLeft != null) v.waitLeft = lerp(a.waitLeft, b.waitLeft, k);
    if (a.temp != null && b.temp != null) v.temp = lerp(a.temp, b.temp, k);
    return v;
  }
}

/**
 * Eases a displayed value toward its target: an exponential (critically
 * damped) approach with time constant `tauMs`, independent of frame rate.
 * Steps larger than `snap` (a new run resetting to 0 g, a re-home) are taken
 * at once, so the display never shows values that never happened.
 */
export class Smooth {
  private v: number | null = null;

  constructor(
    private tauMs: number,
    private snap: number,
  ) {}

  next(target: number, dtMs: number): number {
    if (this.v == null || Math.abs(target - this.v) > this.snap || dtMs <= 0) return (this.v = target);
    this.v += (target - this.v) * (1 - Math.exp(-dtMs / this.tauMs));
    return this.v;
  }

  reset() {
    this.v = null;
  }
}
