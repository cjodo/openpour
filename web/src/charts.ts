// Live charts for brewing and calibration: time charts (water poured, flow
// rate) drawn from one shared history, and a top-down view of the nozzle's
// position over the dripper. Canvas, no libraries; colours come from CSS
// tokens so both themes work.

export interface Sample {
  /** Seconds since the run started. */
  t: number;
  poured: number;
  flow: number;
  /** What the pump is asked for; null when nothing is (calibration, priming). */
  targetFlow: number | null;
}

export interface SeriesDef {
  label: string;
  /** CSS custom property holding the colour. */
  color: string;
  value: (s: Sample) => number | null;
  dashed?: boolean;
  /** Hold each value until the next (a set-point, not a measurement). */
  step?: boolean;
  /** A 10% wash under the line. */
  fill?: boolean;
}

export interface TimeChartOptions {
  series: SeriesDef[];
  unit: string;
  decimals: number;
  /** The y axis covers at least this much. */
  minTop: number;
  /** Horizontal reference lines (e.g. stage targets), drawn under the data. */
  references?: () => number[];
  /** Expected run length, so the x axis doesn't rescale every second. */
  spanHint?: () => number;
}

const css = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

/** 0, then 1/2/5 × 10^n steps giving about `n` ticks up to `top`. */
function niceStep(top: number, n = 3) {
  const raw = top / n;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const f = raw / mag;
  return (f <= 1 ? 1 : f <= 2 ? 2 : f <= 5 ? 5 : 10) * mag;
}

const clock = (s: number) => `${Math.floor(s / 60)}:${String(Math.round(s % 60)).padStart(2, '0')}`;

function fit(canvas: HTMLCanvasElement) {
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth;
  const h = canvas.clientHeight;
  if (canvas.width !== Math.round(w * dpr)) canvas.width = Math.round(w * dpr);
  if (canvas.height !== Math.round(h * dpr)) canvas.height = Math.round(h * dpr);
  const g = canvas.getContext('2d');
  g?.setTransform(dpr, 0, 0, dpr, 0, 0);
  return { g, w, h };
}

/** A filled marker with a 2px surface ring, so it stays legible over lines. */
function dot(g: CanvasRenderingContext2D, x: number, y: number, color: string, surface: string, r = 4) {
  g.beginPath();
  g.arc(x, y, r + 2, 0, Math.PI * 2);
  g.fillStyle = surface;
  g.fill();
  g.beginPath();
  g.arc(x, y, r, 0, Math.PI * 2);
  g.fillStyle = color;
  g.fill();
}

const PAD = { left: 36, right: 10, top: 8, bottom: 18 };

export class TimeChart {
  private samples: Sample[] = [];
  private hoverX: number | null = null;

  constructor(
    private canvas: HTMLCanvasElement,
    private tip: HTMLElement,
    private opts: TimeChartOptions,
  ) {
    const move = (e: PointerEvent) => {
      const r = canvas.getBoundingClientRect();
      this.hoverX = e.clientX - r.left;
      this.draw(this.samples);
    };
    canvas.addEventListener('pointermove', move);
    canvas.addEventListener('pointerdown', move);
    canvas.addEventListener('pointerleave', () => {
      this.hoverX = null;
      this.tip.hidden = true;
      this.draw(this.samples);
    });
  }

  draw(samples: Sample[]) {
    this.samples = samples;
    if (!this.canvas.offsetParent) return; // hidden tab
    const { g, w, h } = fit(this.canvas);
    if (!g) return;
    g.clearRect(0, 0, w, h);
    const { series } = this.opts;
    const ink = css('--ink-soft');
    const grid = css('--line');
    const surface = css('--surface');
    const font = css('--font');

    const lastT = samples.at(-1)?.t ?? 0;
    const span = Math.max(30, this.opts.spanHint?.() ?? 0, lastT * 1.05);
    let peak = this.opts.minTop;
    for (const s of samples) for (const d of series) peak = Math.max(peak, d.value(s) ?? 0);
    for (const r of this.opts.references?.() ?? []) peak = Math.max(peak, r);
    const step = niceStep(peak * 1.1);
    const top = Math.ceil((peak * 1.1) / step) * step;

    const pw = w - PAD.left - PAD.right;
    const ph = h - PAD.top - PAD.bottom;
    const x = (t: number) => PAD.left + (t / span) * pw;
    const y = (v: number) => PAD.top + ph - (v / top) * ph;

    // Recessive axes: hairline, solid.
    g.font = `11px ${font}`;
    g.lineWidth = 1;
    g.strokeStyle = grid;
    g.fillStyle = ink;
    g.textBaseline = 'middle';
    g.textAlign = 'right';
    for (let v = 0; v <= top + 1e-9; v += step) {
      g.beginPath();
      g.moveTo(PAD.left, Math.round(y(v)) + 0.5);
      g.lineTo(w - PAD.right, Math.round(y(v)) + 0.5);
      g.stroke();
      g.fillText(v.toFixed(step < 1 ? 1 : 0), PAD.left - 6, y(v));
    }
    g.textAlign = 'center';
    g.textBaseline = 'alphabetic';
    const tStep = span <= 90 ? 15 : span <= 240 ? 30 : 60;
    for (let t = 0; t <= span; t += tStep) {
      g.fillText(clock(t), Math.min(Math.max(x(t), PAD.left + 12), w - 16), h - 3);
    }

    // Reference lines (stage targets).
    g.strokeStyle = ink;
    g.setLineDash([2, 4]);
    for (const r of this.opts.references?.() ?? []) {
      g.beginPath();
      g.moveTo(PAD.left, Math.round(y(r)) + 0.5);
      g.lineTo(w - PAD.right, Math.round(y(r)) + 0.5);
      g.stroke();
    }
    g.setLineDash([]);

    // Data: 2px lines, round joins.
    g.lineWidth = 2;
    g.lineJoin = 'round';
    g.lineCap = 'round';
    for (const d of series) {
      const color = css(d.color);
      const pts: [number, number][] = [];
      let prev: number | null = null;
      for (const s of samples) {
        const v = d.value(s);
        if (v == null) continue;
        if (d.step && prev != null) pts.push([x(s.t), y(prev)]);
        pts.push([x(s.t), y(v)]);
        prev = v;
      }
      if (pts.length < 2) continue;
      if (d.fill) {
        g.beginPath();
        g.moveTo(pts[0][0], y(0));
        for (const [px, py] of pts) g.lineTo(px, py);
        g.lineTo(pts.at(-1)![0], y(0));
        g.closePath();
        g.globalAlpha = 0.1;
        g.fillStyle = color;
        g.fill();
        g.globalAlpha = 1;
      }
      g.strokeStyle = color;
      g.setLineDash(d.dashed ? [5, 4] : []);
      g.beginPath();
      pts.forEach(([px, py], i) => (i ? g.lineTo(px, py) : g.moveTo(px, py)));
      g.stroke();
      g.setLineDash([]);
      if (!d.dashed && this.hoverX == null) {
        const [ex, ey] = pts.at(-1)!;
        dot(g, ex, ey, color, surface);
      }
    }

    this.drawHover(g, x, y, h, surface);
  }

  /** Crosshair snapped to the nearest sample, with every series' value. */
  private drawHover(
    g: CanvasRenderingContext2D,
    x: (t: number) => number,
    y: (v: number) => number,
    h: number,
    surface: string,
  ) {
    if (this.hoverX == null || !this.samples.length) {
      this.tip.hidden = true;
      return;
    }
    let near = this.samples[0];
    for (const s of this.samples) if (Math.abs(x(s.t) - this.hoverX) < Math.abs(x(near.t) - this.hoverX)) near = s;
    const cx = Math.round(x(near.t)) + 0.5;
    g.strokeStyle = css('--ink-soft');
    g.lineWidth = 1;
    g.beginPath();
    g.moveTo(cx, PAD.top);
    g.lineTo(cx, h - PAD.bottom);
    g.stroke();

    this.tip.replaceChildren();
    const time = document.createElement('div');
    time.className = 'tip-time';
    time.textContent = clock(near.t);
    this.tip.append(time);
    for (const d of this.opts.series) {
      const v = d.value(near);
      if (v == null) continue;
      dot(g, cx, y(v), css(d.color), surface);
      const row = document.createElement('div');
      row.className = 'tip-row';
      const key = document.createElement('span');
      key.className = `key${d.dashed ? ' dashed' : ''}`;
      key.style.setProperty('--key', `var(${d.color})`);
      const value = document.createElement('strong');
      value.textContent = `${v.toFixed(this.opts.decimals)} ${this.opts.unit}`;
      const label = document.createElement('span');
      label.textContent = d.label;
      row.append(key, value, label);
      this.tip.append(row);
    }
    this.tip.hidden = false;
    const wrap = this.canvas.clientWidth;
    const tw = this.tip.offsetWidth;
    this.tip.style.left = `${Math.min(Math.max(cx - tw / 2, 0), wrap - tw)}px`;
  }
}

// ---------------------------------------------------------------- nozzle

/** One animated point of the nozzle's path; `at` is when it was drawn (ms). */
export interface TrailPoint {
  x: number;
  y: number;
  at: number;
}

/** How long the path stays visible, fading out. */
export const TRAIL_MS = 6000;

/**
 * Top-down view oriented like the jog pad on the Machine tab: ▲ (out, away
 * from the pivot, +x) is up and ▶ (swing right, +y) is right, so the dot
 * moves the way the buttons do. The cross marks the saved dripper centre.
 */
export class NozzleChart {
  constructor(
    private canvas: HTMLCanvasElement,
    private readout: HTMLElement,
  ) {}

  draw(trail: TrailPoint[], now: number, current: [number, number] | null, ring: number | null) {
    if (current) {
      const [x, y] = current;
      this.readout.textContent =
        `${Math.hypot(x, y).toFixed(1)} mm from the centre (x ${x.toFixed(1)}, y ${y.toFixed(1)})`;
    } else {
      this.readout.textContent = 'Position unknown: home the arm first.';
    }
    if (!this.canvas.offsetParent) return;
    const { g, w, h } = fit(this.canvas);
    if (!g) return;
    g.clearRect(0, 0, w, h);
    const ink = css('--ink-soft');
    const grid = css('--line');
    const surface = css('--surface');
    const color = css('--series-flow');
    const font = css('--font');

    // Scale to where the nozzle is and the pattern, not to old travel moves
    // (they'd zoom out too far to centre by); older path is clipped instead.
    const head0 = trail.at(-1) ?? (current ? { x: current[0], y: current[1] } : null);
    let reach = Math.max(30, (ring ?? 0) + 8, head0 ? Math.hypot(head0.x, head0.y) + 8 : 0);
    reach = Math.ceil(reach / 10) * 10;
    const cx = w / 2;
    const cy = h / 2;
    const k = (Math.min(w, h) / 2 - 14) / reach;
    const sx = (x: number, y: number): [number, number] => [cx + y * k, cy - x * k];

    // Rings every 10 mm and the axes: hairline, recessive.
    g.lineWidth = 1;
    g.strokeStyle = grid;
    for (let r = 10; r <= reach; r += 10) {
      g.beginPath();
      g.arc(cx, cy, r * k, 0, Math.PI * 2);
      g.stroke();
    }
    g.beginPath();
    g.moveTo(cx - reach * k, Math.round(cy) + 0.5);
    g.lineTo(cx + reach * k, Math.round(cy) + 0.5);
    g.moveTo(Math.round(cx) + 0.5, cy - reach * k);
    g.lineTo(Math.round(cx) + 0.5, cy + reach * k);
    g.stroke();
    g.font = `11px ${font}`;
    g.fillStyle = ink;
    g.textAlign = 'left';
    g.fillText(`${reach} mm`, cx + reach * k * 0.71 + 4, cy - reach * k * 0.71);
    g.textAlign = 'center';
    g.fillText('out', cx, 11);
    g.fillText('pivot', cx, h - 3);

    // The pattern being poured, as a dashed guide.
    if (ring && ring > 0) {
      g.strokeStyle = ink;
      g.setLineDash([4, 4]);
      g.beginPath();
      g.arc(cx, cy, ring * k, 0, Math.PI * 2);
      g.stroke();
      g.setLineDash([]);
    }

    // Saved centre.
    g.strokeStyle = css('--ink');
    g.lineWidth = 2;
    g.beginPath();
    g.moveTo(cx - 6, cy);
    g.lineTo(cx + 6, cy);
    g.moveTo(cx, cy - 6);
    g.lineTo(cx, cy + 6);
    g.stroke();

    // The path, fading with age, clipped to the view.
    g.save();
    g.beginPath();
    g.arc(cx, cy, reach * k + 4, 0, Math.PI * 2);
    g.clip();
    g.strokeStyle = color;
    g.lineWidth = 2;
    g.lineJoin = 'round';
    g.lineCap = 'round';
    for (let i = 1; i < trail.length; i++) {
      const age = (now - trail[i].at) / TRAIL_MS;
      if (age >= 1) continue;
      g.globalAlpha = 1 - age;
      g.beginPath();
      g.moveTo(...sx(trail[i - 1].x, trail[i - 1].y));
      g.lineTo(...sx(trail[i].x, trail[i].y));
      g.stroke();
    }
    g.globalAlpha = 1;
    g.restore();

    const head = trail.at(-1) ?? (current ? { x: current[0], y: current[1] } : null);
    if (head) dot(g, ...sx(head.x, head.y), color, surface, 5);
  }
}
