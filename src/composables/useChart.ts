export type Unit = "percent" | "bytes" | "rate" | "bits" | "count" | "number" | "ops" | "celsius" | "duration";
export type Point = [number, number, number, number];
export type Line = { id: string; label: string; color: string; points: Point[]; dashed?: boolean; hidden?: boolean };
export type Span = { from: number; to: number; step: number };
export type ChartOptions = { title: string; unit: Unit; log?: boolean };

const FONT = '11px "Commit Mono", ui-monospace, monospace';
const PAD = { top: 10, right: 12, bottom: 28 };
const GUTTER = 72;
const X_STEPS = [1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 10800, 21600, 43200, 86400, 172800, 604800, 1209600, 2592000];

function pad(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

function clock(date: Date, seconds: boolean): string {
  return `${pad(date.getHours())}:${pad(date.getMinutes())}${seconds ? `:${pad(date.getSeconds())}` : ""}`;
}

function day(date: Date): string {
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export function formatTime(t: number): string {
  const date = new Date(t * 1000);
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${clock(date, true)}`;
}

function trim(v: number, digits: number): string {
  return v.toFixed(digits).replace(/\.0+$|(\.\d*?)0+$/, "$1");
}

function scaled(v: number, base: number, units: string[]): string {
  let value = Math.abs(v);
  let i = 0;
  while (value >= base && i < units.length - 1) {
    value /= base;
    i++;
  }
  const sign = v < 0 ? "-" : "";
  return `${sign}${trim(value, value >= 100 || (i === 0 && !(units[0] === "/s" && value < 10)) ? 0 : value >= 10 ? 1 : 2)} ${units[i]}`;
}

function duration(v: number): string {
  let rest = Math.round(Math.abs(v));
  const parts: string[] = [];
  for (const [size, name] of [[86400, "d"], [3600, "h"], [60, "m"], [1, "s"]] as [number, string][]) {
    if (rest >= size) {
      parts.push(`${Math.floor(rest / size)}${name}`);
      rest %= size;
    }
    if (parts.length === 2) break;
  }
  return parts.length ? parts.join(" ") : "0s";
}

export function formatValue(v: number, unit: Unit): string {
  switch (unit) {
    case "ops":
      return scaled(v, 1000, ["/s", "k/s", "M/s", "G/s"]);
    case "celsius":
      return `${trim(v, 1)} °C`;
    case "duration":
      return duration(v);
    case "percent":
      return `${trim(v, v >= 100 ? 0 : 1)}%`;
    case "bytes":
      return scaled(v, 1024, ["B", "KiB", "MiB", "GiB", "TiB"]);
    case "rate":
      return scaled(v, 1024, ["B/s", "KiB/s", "MiB/s", "GiB/s", "TiB/s"]);
    case "bits":
      return scaled(v, 1000, ["bit/s", "kbit/s", "Mbit/s", "Gbit/s", "Tbit/s"]);
    case "count":
      return String(Math.round(v));
    default:
      return trim(v, Math.abs(v) >= 100 ? 0 : 2);
  }
}

function niceStep(raw: number): number {
  const power = 10 ** Math.floor(Math.log10(raw));
  const f = raw / power;
  return (f <= 1 ? 1 : f <= 2 ? 2 : f <= 5 ? 5 : 10) * power;
}

type Scale = { ticks: number[]; at: (v: number) => number; log: boolean };

const MIN_SPAN: Record<Unit, number> = { percent: 5, celsius: 5, count: 4, number: 1, ops: 0.1, bytes: 1024, rate: 1024, bits: 1000, duration: 60 };
const WHOLE: Unit[] = ["bytes", "rate", "bits", "count", "duration"];

function tickStep(span: number, unit: Unit): number {
  const raw = span / 4;
  if (unit === "bytes" || unit === "rate") return Math.max(1, 2 ** Math.ceil(Math.log2(raw)));
  let scale = 1;
  if (unit === "bits" || unit === "ops") while (raw / scale >= 1000) scale *= 1000;
  else if (unit === "duration") scale = [86400, 3600, 60, 1].find((u) => raw >= u) ?? 1;
  const step = niceStep(raw / scale) * scale;
  return WHOLE.includes(unit) ? Math.max(1, Math.round(step)) : step;
}

function linearScale(min: number, max: number, unit: Unit): Scale {
  let lo = min;
  let hi = max;
  const least = Math.max(MIN_SPAN[unit], hi * 0.05);
  if (hi - lo < least) {
    const mid = (lo + hi) / 2;
    lo = mid - least / 2;
    hi = mid + least / 2;
  }
  const room = (hi - lo) * 0.08;
  const ceiling = unit === "percent" && max <= 100 ? 100 : Infinity;
  lo = min < max * 0.25 ? 0 : Math.max(0, lo - room);
  hi = Math.min(ceiling, hi + room);
  const step = tickStep(hi - lo, unit);
  const first = Math.floor(lo / step);
  const last = Math.max(first + 1, Math.ceil(hi / step - 1e-9));
  const ticks: number[] = [];
  for (let i = first; i <= last; i++) ticks.push(i * step);
  const bottom = first * step;
  const size = last * step - bottom;
  return { ticks, at: (v) => (v - bottom) / size, log: false };
}

function logScale(min: number, smallest: number, max: number): Scale {
  const top = Math.max(2, Math.ceil(Math.log2(max * 1.02)));
  const low = Math.min(top - 2, Math.max(Math.floor(Math.log2(smallest)), top - 20, 0));
  const stride = Math.ceil((top - low) / 5);
  const floor = 2 ** low;
  const base = min < floor ? stride : 0;
  const ticks: number[] = [];
  for (let e = top; e >= low; e -= stride) ticks.unshift(2 ** e);
  if (base) ticks.unshift(0);
  else if (Math.log2(ticks[0]!) - low >= stride / 2) ticks.unshift(floor);
  const size = top - low + base;
  const at = (v: number) => {
    if (v >= floor) return (Math.log2(v) - low + base) / size;
    return base ? ((Math.max(v, 0) / floor) * base) / size : -1;
  };
  return { ticks, at, log: true };
}

function yScale(lines: Line[], from: number, to: number, unit: Unit, log: boolean): Scale {
  const extent = (set: Line[]) => {
    let min = Infinity;
    let max = -Infinity;
    let smallest = Infinity;
    for (const line of set)
      for (const p of line.points) {
        if (p[0] < from || p[0] > to) continue;
        if (p[2] < min) min = p[2];
        if (p[3] > max) max = p[3];
        for (const v of [p[1], p[2]]) if (v > 0 && v < smallest) smallest = v;
      }
    return { min, max, smallest };
  };
  const data = extent(lines.filter((l) => !l.dashed));
  const refs = extent(lines.filter((l) => l.dashed));
  let { min, max, smallest } = data;
  if (!(max > 0) || refs.max <= max * 1.5) {
    min = Math.min(min, refs.min);
    max = Math.max(max, refs.max);
    smallest = Math.min(smallest, refs.smallest);
  }
  if (log && smallest < Infinity && max >= Math.max(1, smallest) * 8) return logScale(Math.max(0, min), smallest, max);
  if (max === -Infinity) return linearScale(0, 0, unit);
  return linearScale(Math.max(0, min), Math.max(0, max), unit);
}

function xTicks(span: Span, width: number): { t: number; label: string }[] {
  const range = span.to - span.from;
  const wanted = Math.max(2, Math.floor(width / 90));
  const step = X_STEPS.find((s) => range / s <= wanted) ?? X_STEPS[X_STEPS.length - 1]!;
  const offset = -new Date(span.from * 1000).getTimezoneOffset() * 60;
  const first = Math.ceil((span.from + offset) / step) * step - offset;
  const dated = range > 36 * 3600;
  const out: { t: number; label: string }[] = [];
  for (let t = first; t <= span.to; t += step) {
    const date = new Date(t * 1000);
    let label: string;
    if (step >= 86400) label = day(date);
    else if (dated) label = `${day(date)} ${clock(date, false)}`;
    else label = clock(date, step < 60);
    out.push({ t, label });
  }
  return out;
}

function nearest(points: Point[], t: number): Point | null {
  let lo = 0;
  let hi = points.length - 1;
  if (hi < 0) return null;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (points[mid]![0] < t) lo = mid + 1;
    else hi = mid;
  }
  const a = points[lo]!;
  const b = points[lo - 1];
  return b && Math.abs(b[0] - t) < Math.abs(a[0] - t) ? b : a;
}

type Card = { el: HTMLElement; swatch: HTMLElement; label: HTMLElement; last: HTMLElement; cells: Text[]; line: Line };
type Geo = { x0: number; x1: number; y0: number; y1: number; Y: (v: number) => number; from: number; to: number; muted: string; text: string };

export class Chart {
  readonly el: HTMLElement;
  private plot: HTMLElement;
  private canvas: HTMLCanvasElement;
  private overlay: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private octx: CanvasRenderingContext2D;
  private tip: HTMLElement;
  private noteEl: HTMLElement;
  private emptyEl: HTMLElement;
  private scaleEl: HTMLElement;
  private legend: HTMLElement;
  private cards = new Map<string, Card>();
  private resort = true;
  private local = false;
  private tipHead: HTMLElement | null = null;
  private tipRows = new Map<string, { row: HTMLElement; value: HTMLElement; band: HTMLElement | null }>();
  private tipOrder = "";
  private tipV = 0;
  private tipB = 0;
  private tipKey = "";
  private tipW = 0;
  private tipH = 0;
  private tipSide: "r" | "l" = "r";
  private lines: Line[] = [];
  private span: Span = { from: 0, to: 1, step: 1 };
  private cursor: number | null = null;
  private drag: { x0: number; x1: number } | null = null;
  private queued = false;
  private overlayQueued = false;
  private geo: Geo | null = null;
  private left = GUTTER;
  private awayBound = false;
  readonly unit: Unit;
  private log: boolean;
  onCursor: (t: number | null) => void = () => {};
  onZoom: (from: number, to: number) => void = () => {};
  onZoomOut: () => void = () => {};

  constructor(parent: HTMLElement, options: ChartOptions) {
    this.unit = options.unit;
    this.log = options.log ?? false;
    this.el = document.createElement("section");
    this.el.className = "chart";
    const head = document.createElement("header");
    const title = document.createElement("h3");
    title.textContent = options.title;
    this.emptyEl = document.createElement("span");
    this.emptyEl.className = "chart-empty muted";
    this.emptyEl.hidden = true;
    this.scaleEl = document.createElement("span");
    this.scaleEl.className = "chart-scale muted";
    this.scaleEl.textContent = "log₂";
    this.scaleEl.hidden = true;
    this.legend = document.createElement("div");
    this.legend.className = "chart-stats";
    head.append(title, this.emptyEl, this.scaleEl);
    this.plot = document.createElement("div");
    this.plot.className = "chart-plot";
    this.canvas = document.createElement("canvas");
    this.overlay = document.createElement("canvas");
    this.overlay.className = "chart-overlay";
    this.tip = document.createElement("div");
    this.tip.className = "chart-tip";
    this.tip.hidden = true;
    this.noteEl = document.createElement("p");
    this.noteEl.className = "chart-note muted";
    this.noteEl.hidden = true;
    this.plot.append(this.canvas, this.overlay, this.tip, this.noteEl);
    this.el.append(head, this.plot, this.legend);
    parent.append(this.el);
    this.ctx = this.canvas.getContext("2d")!;
    this.octx = this.overlay.getContext("2d")!;
    new ResizeObserver(() => this.resize()).observe(this.plot);
    this.bind();
    this.resize();
  }

  get width(): number {
    return Math.max(0, this.plot.clientWidth - this.left - PAD.right);
  }

  setLines(lines: Line[], span: Span, resort = true) {
    this.lines = lines;
    this.span = span;
    if (resort) this.resort = true;
    this.draw();
  }

  setSpan(span: Span) {
    this.span = span;
    this.draw();
  }

  note(text: string | null) {
    this.noteEl.hidden = !text;
    this.noteEl.textContent = text ?? "";
  }

  empty(text: string | null) {
    this.el.classList.toggle("no-data", text !== null);
    this.emptyEl.hidden = text === null;
    if (text !== null) this.scaleEl.hidden = true;
    this.emptyEl.textContent = text ?? "";
  }

  setCursor(fraction: number | null) {
    this.cursor = fraction;
    this.drawOverlay();
  }

  updateLegend() {
    this.resort = true;
    this.draw();
  }

  private renderLegend(resort: boolean) {
    if (!this.lines.length) {
      this.legend.replaceChildren();
      this.cards.clear();
      return;
    }
    const live = new Set(this.lines.map((l) => l.id));
    for (const [id, card] of this.cards) {
      if (live.has(id)) continue;
      card.el.remove();
      this.cards.delete(id);
    }
    const show = (v: number | null) => (v === null ? "–" : formatValue(v, this.unit));
    const entries: { card: Card; max: number }[] = [];
    const fresh: HTMLElement[] = [];
    for (const line of this.lines) {
      let card = this.cards.get(line.id);
      if (!card) {
        card = this.createCard(line);
        this.cards.set(line.id, card);
        fresh.push(card.el);
      }
      card.line = line;
      const stats = this.stats(line);
      const text = (node: Text | HTMLElement, value: string) => {
        if (node instanceof Text) {
          if (node.data !== value) node.data = value;
        } else if (node.textContent !== value) node.textContent = value;
      };
      card.el.classList.toggle("off", !!line.hidden);
      if (card.el.title !== line.label) card.el.title = line.label;
      if (card.swatch.style.background !== line.color) card.swatch.style.background = line.color;
      text(card.label, line.label);
      text(card.last, show(stats[0] ?? null));
      [stats[1]!, stats[2]!, stats[3]!].forEach((v, i) => text(card.cells[i]!, show(v)));
      entries.push({ card, max: stats[3] ?? -Infinity });
    }
    if (resort) {
      entries.sort((a, b) => b.max - a.max);
      this.legend.append(...entries.map((e) => e.card.el));
    } else if (fresh.length) {
      this.legend.append(...fresh);
    }
  }

  private createCard(line: Line): Card {
    const el = document.createElement("button");
    el.type = "button";
    el.className = "chart-card";
    const head = document.createElement("span");
    head.className = "chart-card-head";
    const swatch = document.createElement("i");
    const label = document.createElement("span");
    head.append(swatch, label);
    const last = document.createElement("b");
    const range = document.createElement("span");
    range.className = "chart-card-range";
    const cells: Text[] = [];
    for (const name of ["Min", "Avg", "Max"]) {
      const cell = document.createElement("span");
      const small = document.createElement("small");
      small.textContent = name;
      const value = document.createTextNode("");
      cell.append(small, value);
      range.append(cell);
      cells.push(value);
    }
    el.append(head, last, range);
    const card: Card = { el, swatch, label, last, cells, line };
    el.addEventListener("click", () => {
      card.line.hidden = !card.line.hidden;
      this.renderLegend(false);
      this.draw();
    });
    return card;
  }

  private stats(line: Line): (number | null)[] {
    let min = Infinity;
    let max = -Infinity;
    let sum = 0;
    let n = 0;
    let last: number | null = null;
    for (const p of line.points) {
      if (p[0] < this.span.from || p[0] > this.span.to) continue;
      if (p[2] < min) min = p[2];
      if (p[3] > max) max = p[3];
      sum += p[1];
      n++;
      last = p[1];
    }
    return n ? [last, min, sum / n, max] : [null, null, null, null];
  }

  private resize() {
    const w = this.plot.clientWidth;
    const h = this.plot.clientHeight;
    if (!w || !h) return;
    const dpr = window.devicePixelRatio || 1;
    for (const [canvas, ctx] of [[this.canvas, this.ctx], [this.overlay, this.octx]] as [HTMLCanvasElement, CanvasRenderingContext2D][]) {
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      canvas.style.width = `${w}px`;
      canvas.style.height = `${h}px`;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }
    this.paint();
    this.paintOverlay();
  }

  private bind() {
    const canvas = this.overlay;
    const time = (event: PointerEvent): number => {
      const rect = canvas.getBoundingClientRect();
      const x = Math.min(Math.max(event.clientX - rect.left, this.left), this.left + this.width);
      return (x - this.left) / Math.max(1, this.width);
    };
    const localX = (event: PointerEvent): number => event.clientX - canvas.getBoundingClientRect().left;
    canvas.addEventListener("pointermove", (event) => {
      const t = time(event);
      if (this.drag) {
        this.drag.x1 = localX(event);
        this.drawOverlay();
        return;
      }
      this.local = true;
      this.cursor = t;
      this.drawOverlay();
      this.onCursor(t);
    });
    canvas.addEventListener("pointerleave", (event) => {
      if (this.drag || event.pointerType === "touch") return;
      this.local = false;
      this.cursor = null;
      this.drawOverlay();
      this.onCursor(null);
    });
    canvas.addEventListener("pointerdown", (event) => {
      if (event.pointerType === "touch") {
        this.local = true;
        this.cursor = time(event);
        this.drawOverlay();
        this.onCursor(this.cursor);
        if (this.awayBound) return;
        this.awayBound = true;
        const away = (e: PointerEvent) => {
          if (canvas.contains(e.target as Node)) return;
          document.removeEventListener("pointerdown", away, true);
          this.awayBound = false;
          this.local = false;
          this.cursor = null;
          this.drawOverlay();
          this.onCursor(null);
        };
        document.addEventListener("pointerdown", away, true);
        return;
      }
      if (event.button !== 0) return;
      canvas.setPointerCapture(event.pointerId);
      const x = localX(event);
      this.drag = { x0: x, x1: x };
    });
    canvas.addEventListener("pointerup", (event) => {
      const drag = this.drag;
      this.drag = null;
      canvas.releasePointerCapture(event.pointerId);
      if (!drag) return;
      const a = Math.min(drag.x0, drag.x1);
      const b = Math.max(drag.x0, drag.x1);
      this.drawOverlay();
      if (b - a < 8) return;
      const range = this.span.to - this.span.from;
      const at = (x: number) => this.span.from + ((Math.min(Math.max(x, this.left), this.left + this.width) - this.left) / Math.max(1, this.width)) * range;
      this.onZoom(at(a), at(b));
    });
    canvas.addEventListener("pointercancel", () => {
      this.drag = null;
      this.drawOverlay();
    });
    canvas.addEventListener("dblclick", () => this.onZoomOut());
  }

  draw() {
    if (this.queued) return;
    this.queued = true;
    requestAnimationFrame(() => {
      this.queued = false;
      this.renderLegend(this.resort);
      this.resort = false;
      this.paint();
      this.paintOverlay();
    });
  }

  private drawOverlay() {
    if (this.overlayQueued || this.queued) return;
    this.overlayQueued = true;
    requestAnimationFrame(() => {
      this.overlayQueued = false;
      this.paintOverlay();
    });
  }

  private paint() {
    const w = this.plot.clientWidth;
    const h = this.plot.clientHeight;
    if (!w || !h) return;
    const ctx = this.ctx;
    const style = getComputedStyle(this.el);
    const muted = style.getPropertyValue("--text-muted").trim() || "#8d8986";
    const grid = style.getPropertyValue("--grid").trim() || "rgb(255 255 255 / 7%)";
    const text = style.getPropertyValue("--text").trim() || "#ebe9e7";
    ctx.clearRect(0, 0, w, h);
    ctx.font = FONT;

    const { from, to, step } = this.span;
    const visible = this.lines.filter((l) => !l.hidden);
    const scale = yScale(visible, from, to, this.unit, this.log);
    const ticks = scale.ticks;
    this.scaleEl.hidden = !scale.log;
    const labels = ticks.map((v) => formatValue(v, this.unit));
    this.left = Math.ceil(Math.max(0, ...labels.map((l) => ctx.measureText(l).width))) + 10;
    const x0 = this.left;
    const x1 = w - PAD.right;
    const y0 = PAD.top;
    const y1 = h - PAD.bottom;
    const range = Math.max(1e-9, to - from);
    const X = (t: number) => x0 + ((t - from) / range) * (x1 - x0);
    const Y = (v: number) => y1 - scale.at(v) * (y1 - y0);
    this.geo = { x0, x1, y0, y1, Y, from, to, muted, text };

    ctx.lineWidth = 1;
    ctx.strokeStyle = grid;
    ctx.fillStyle = muted;
    ctx.textBaseline = "middle";
    ctx.textAlign = "right";
    ticks.forEach((v, i) => {
      const y = Math.round(Y(v)) + 0.5;
      ctx.beginPath();
      ctx.moveTo(x0, y);
      ctx.lineTo(x1, y);
      ctx.stroke();
      ctx.fillText(labels[i]!, x0 - 8, y);
    });
    ctx.textAlign = "center";
    ctx.textBaseline = "top";
    for (const tick of xTicks(this.span, x1 - x0)) {
      const x = Math.round(X(tick.t)) + 0.5;
      ctx.beginPath();
      ctx.moveTo(x, y1);
      ctx.lineTo(x, y1 + 4);
      ctx.stroke();
      const half = ctx.measureText(tick.label).width / 2;
      if (x - half >= 0 && x + half <= w) ctx.fillText(tick.label, x, y1 + 10);
    }

    ctx.save();
    ctx.beginPath();
    ctx.rect(x0, y0 - 2, x1 - x0, y1 - y0 + 4);
    ctx.clip();
    const gap = Math.max(step * 2.5, 5);
    for (const line of visible) {
      const segments: Point[][] = [];
      let current: Point[] = [];
      for (const p of line.points) {
        if (p[0] < from - step || p[0] > to + step) continue;
        const prev = current[current.length - 1];
        if (prev && p[0] - prev[0] > gap) {
          segments.push(current);
          current = [];
        }
        current.push(p);
      }
      if (current.length) segments.push(current);
      for (const seg of segments) {
        if (seg.length === 1) {
          ctx.fillStyle = line.color;
          ctx.beginPath();
          ctx.arc(X(seg[0]![0]), Y(seg[0]![1]), 2, 0, Math.PI * 2);
          ctx.fill();
          continue;
        }
        if (step > 1 && seg.some((p) => p[3] > p[2])) {
          ctx.globalAlpha = 0.22;
          ctx.fillStyle = line.color;
          ctx.beginPath();
          seg.forEach((p, i) => (i ? ctx.lineTo(X(p[0]), Y(p[3])) : ctx.moveTo(X(p[0]), Y(p[3]))));
          for (let i = seg.length - 1; i >= 0; i--) ctx.lineTo(X(seg[i]![0]), Y(seg[i]![2]));
          ctx.closePath();
          ctx.fill();
          ctx.globalAlpha = 1;
        }
        ctx.strokeStyle = line.color;
        ctx.lineWidth = 1.5;
        ctx.lineJoin = "round";
        ctx.setLineDash(line.dashed ? [5, 4] : []);
        ctx.beginPath();
        seg.forEach((p, i) => (i ? ctx.lineTo(X(p[0]), Y(p[1])) : ctx.moveTo(X(p[0]), Y(p[1]))));
        ctx.stroke();
        ctx.setLineDash([]);
      }
    }
    ctx.restore();
  }

  private paintOverlay() {
    const w = this.plot.clientWidth;
    const h = this.plot.clientHeight;
    const geo = this.geo;
    if (!w || !h || !geo) return;
    const ctx = this.octx;
    ctx.clearRect(0, 0, w, h);
    const { x0, x1, y0, y1, Y, from, to, muted, text } = geo;
    const range = Math.max(1e-9, to - from);
    const X = (t: number) => x0 + ((t - from) / range) * (x1 - x0);

    if (this.drag) {
      const a = Math.min(this.drag.x0, this.drag.x1);
      const b = Math.max(this.drag.x0, this.drag.x1);
      const left = Math.max(a, x0);
      const right = Math.min(b, x1);
      ctx.fillStyle = text;
      ctx.globalAlpha = 0.1;
      ctx.fillRect(left, y0, Math.max(0, right - left), y1 - y0);
      ctx.globalAlpha = 1;
    }

    if (this.cursor === null) {
      this.resetTip();
      return;
    }
    const at = from + this.cursor * range;
    const x = Math.round(X(at)) + 0.5;
    ctx.lineWidth = 1;
    ctx.strokeStyle = muted;
    ctx.setLineDash([3, 3]);
    ctx.beginPath();
    ctx.moveTo(x, y0);
    ctx.lineTo(x, y1);
    ctx.stroke();
    ctx.setLineDash([]);
    const rows: { line: Line; p: Point }[] = [];
    for (const line of this.lines) {
      if (line.hidden) continue;
      const p = nearest(line.points, at);
      if (p && Math.abs(p[0] - at) <= Math.max(this.span.step * 1.5, 3)) rows.push({ line, p });
    }
    for (const { line, p } of rows) {
      const y = Y(p[1]);
      if (y < y0 - 2 || y > y1 + 2) continue;
      ctx.fillStyle = line.color;
      ctx.beginPath();
      ctx.arc(X(p[0]), y, 3, 0, Math.PI * 2);
      ctx.fill();
    }
    if (this.local) this.showTip(x, at, rows);
    else this.resetTip();
  }

  private resetTip() {
    this.tipW = 0;
    this.tipH = 0;
    this.tipV = 0;
    this.tipB = 0;
    this.tipSide = "r";
    this.tip.style.minWidth = "";
    this.tip.style.minHeight = "";
    this.tip.style.removeProperty("--tip-v");
    this.tip.style.removeProperty("--tip-b");
    this.tip.hidden = true;
  }

  private showTip(x: number, t: number, rows: { line: Line; p: Point }[]) {
    const visible = this.lines.filter((l) => !l.hidden);
    if (!visible.some((l) => l.points.length)) {
      this.resetTip();
      return;
    }
    const banded = this.span.step > 1;
    const key = `${banded}:${visible.map((l) => l.id).join("|")}`;
    if (key !== this.tipKey || !this.tipHead) {
      this.tipKey = key;
      this.tipOrder = "";
      this.tipW = 0;
      this.tipH = 0;
      this.tipV = 0;
      this.tipB = 0;
      this.tip.style.minWidth = "";
      this.tip.style.minHeight = "";
      this.tip.style.removeProperty("--tip-v");
      this.tip.style.removeProperty("--tip-b");
      this.tip.classList.toggle("banded", banded);
      const head = document.createElement("div");
      head.className = "chart-tip-time";
      this.tipHead = head;
      this.tipRows.clear();
      const nodes: HTMLElement[] = [head];
      for (const line of visible) {
        const row = document.createElement("div");
        row.className = "chart-tip-row";
        const swatch = document.createElement("i");
        swatch.style.background = line.color;
        const label = document.createElement("span");
        label.textContent = line.label;
        const value = document.createElement("b");
        row.append(swatch, label, value);
        let band: HTMLElement | null = null;
        if (banded) {
          band = document.createElement("small");
          row.append(band);
        }
        this.tipRows.set(line.id, { row, value, band });
        nodes.push(row);
      }
      this.tip.replaceChildren(...nodes);
    }
    const data = new Map(rows.map((r) => [r.line.id, r.p]));
    this.tipHead.textContent = formatTime(rows[0]?.p[0] || t);
    const ordered = visible
      .map((line, i) => ({ line, i, v: data.get(line.id)?.[1] ?? -Infinity }))
      .sort((a, b) => b.v - a.v || a.i - b.i)
      .map((e) => e.line);
    const order = ordered.map((l) => l.id).join("|");
    if (order !== this.tipOrder) {
      this.tipOrder = order;
      this.tip.append(...ordered.map((l) => this.tipRows.get(l.id)!.row));
    }
    for (const line of visible) {
      const cell = this.tipRows.get(line.id);
      if (!cell) continue;
      const p = data.get(line.id);
      cell.value.textContent = p ? formatValue(p[1], this.unit) : "–";
      if (cell.band) cell.band.textContent = p ? `${formatValue(p[2], this.unit)} – ${formatValue(p[3], this.unit)}` : "";
    }
    this.tip.hidden = false;
    let v = this.tipV;
    let bw = this.tipB;
    for (const cell of this.tipRows.values()) {
      v = Math.max(v, cell.value.offsetWidth);
      if (cell.band) bw = Math.max(bw, cell.band.offsetWidth);
    }
    if (v > this.tipV) {
      this.tipV = v;
      this.tip.style.setProperty("--tip-v", `${v}px`);
    }
    if (bw > this.tipB) {
      this.tipB = bw;
      this.tip.style.setProperty("--tip-b", `${bw}px`);
    }
    const width = this.tip.offsetWidth;
    const height = this.tip.offsetHeight;
    if (width > this.tipW) {
      this.tipW = width;
      this.tip.style.minWidth = `${width}px`;
    }
    if (height > this.tipH) {
      this.tipH = height;
      this.tip.style.minHeight = `${height}px`;
    }
    const w = Math.max(width, this.tipW);
    const plotWidth = this.plot.clientWidth;
    if (this.tipSide === "r" && x + 14 + w > plotWidth) this.tipSide = "l";
    else if (this.tipSide === "l" && x + 14 + w + 24 <= plotWidth) this.tipSide = "r";
    const left = this.tipSide === "r" ? x + 14 : x - 14 - w;
    this.tip.style.left = `${Math.max(4, Math.min(left, plotWidth - w - 4))}px`;
    this.tip.style.top = `${PAD.top}px`;
  }
}
