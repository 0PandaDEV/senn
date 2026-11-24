export type Unit = "percent" | "bytes" | "rate" | "bits" | "count" | "number" | "ops" | "celsius" | "duration";
export type Point = [number, number, number, number];
export type Line = { id: string; label: string; color: string; points: Point[]; dashed?: boolean; hidden?: boolean };
export type Span = { from: number; to: number; step: number };
export type ChartOptions = { title: string; unit: Unit; yMax?: number };

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
  return `${sign}${trim(value, value >= 100 || i === 0 ? 0 : value >= 10 ? 1 : 2)} ${units[i]}`;
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

function yTicks(max: number, unit: Unit): number[] {
  const top = max > 0 ? max : 1;
  let scale = 1;
  if (unit === "bytes" || unit === "rate") while (top / scale >= 1024) scale *= 1024;
  else if (unit === "bits" || unit === "ops") while (top / scale >= 1000) scale *= 1000;
  else if (unit === "duration") scale = [86400, 3600, 60, 1].find((u) => top / u >= 3) ?? 1;
  let step = niceStep(top / scale / 4) * scale;
  if (unit !== "number" && unit !== "percent" && unit !== "celsius") step = Math.max(1, Math.round(step));
  const ticks: number[] = [];
  for (let v = 0; v < top + step * 0.999; v += step) ticks.push(v);
  return ticks;
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

type Geo = { x0: number; x1: number; y0: number; y1: number; top: number; from: number; to: number; muted: string; text: string };

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
  private legend: HTMLElement;
  private lines: Line[] = [];
  private span: Span = { from: 0, to: 1, step: 1 };
  private cursor: number | null = null;
  private drag: { x0: number; x1: number } | null = null;
  private queued = false;
  private overlayQueued = false;
  private geo: Geo | null = null;
  private readonly left = GUTTER;
  readonly unit: Unit;
  private yMax?: number;
  onCursor: (t: number | null) => void = () => {};
  onZoom: (from: number, to: number) => void = () => {};
  onZoomOut: () => void = () => {};

  constructor(parent: HTMLElement, options: ChartOptions) {
    this.unit = options.unit;
    this.yMax = options.yMax;
    this.el = document.createElement("section");
    this.el.className = "chart";
    const head = document.createElement("header");
    const title = document.createElement("h3");
    title.textContent = options.title;
    this.emptyEl = document.createElement("span");
    this.emptyEl.className = "chart-empty muted";
    this.emptyEl.hidden = true;
    this.legend = document.createElement("div");
    this.legend.className = "chart-stats";
    head.append(title, this.emptyEl);
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

  setLines(lines: Line[], span: Span) {
    this.lines = lines;
    this.span = span;
    this.renderLegend();
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
    this.el.classList.toggle("empty", text !== null);
    this.emptyEl.hidden = text === null;
    this.emptyEl.textContent = text ?? "";
  }

  setCursor(t: number | null) {
    this.cursor = t;
    this.drawOverlay();
  }

  updateLegend() {
    this.renderLegend();
  }

  private renderLegend() {
    this.legend.replaceChildren();
    if (!this.lines.length) return;
    const cards = this.lines.map((line) => ({ line, stats: this.stats(line) }));
    cards.sort((a, b) => (b.stats[3] ?? -Infinity) - (a.stats[3] ?? -Infinity));
    const show = (v: number | null) => (v === null ? "–" : formatValue(v, this.unit));
    for (const { line, stats } of cards) {
      const card = document.createElement("button");
      card.type = "button";
      card.className = "chart-card";
      card.classList.toggle("off", !!line.hidden);
      card.title = line.label;
      const head = document.createElement("span");
      head.className = "chart-card-head";
      const swatch = document.createElement("i");
      swatch.style.background = line.color;
      const label = document.createElement("span");
      label.textContent = line.label;
      head.append(swatch, label);
      const last = document.createElement("b");
      last.textContent = show(stats[0]!);
      const range = document.createElement("span");
      range.className = "chart-card-range";
      for (const [name, value] of [["Min", stats[1]], ["Avg", stats[2]], ["Max", stats[3]]] as [string, number | null][]) {
        const cell = document.createElement("span");
        const small = document.createElement("small");
        small.textContent = name;
        cell.append(small, show(value));
        range.append(cell);
      }
      card.append(head, last, range);
      card.addEventListener("click", () => {
        line.hidden = !line.hidden;
        this.renderLegend();
        this.draw();
      });
      this.legend.append(card);
    }
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
    this.draw();
  }

  private bind() {
    const canvas = this.overlay;
    const time = (event: PointerEvent): number => {
      const rect = canvas.getBoundingClientRect();
      const x = Math.min(Math.max(event.clientX - rect.left, this.left), this.left + this.width);
      return this.span.from + ((x - this.left) / Math.max(1, this.width)) * (this.span.to - this.span.from);
    };
    const localX = (event: PointerEvent): number => event.clientX - canvas.getBoundingClientRect().left;
    canvas.addEventListener("pointermove", (event) => {
      const t = time(event);
      if (this.drag) {
        this.drag.x1 = localX(event);
        this.drawOverlay();
        return;
      }
      this.cursor = t;
      this.drawOverlay();
      this.onCursor(t);
    });
    canvas.addEventListener("pointerleave", () => {
      if (this.drag) return;
      this.cursor = null;
      this.drawOverlay();
      this.onCursor(null);
    });
    canvas.addEventListener("pointerdown", (event) => {
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
      this.paint();
      this.paintOverlay();
    });
  }

  private drawOverlay() {
    if (this.overlayQueued) return;
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
    let max = 0;
    for (const line of visible) for (const p of line.points) if (p[0] >= from && p[0] <= to && p[3] > max) max = p[3];
    const ticks = this.yMax ? yTicks(this.yMax, this.unit) : yTicks(max * 1.05, this.unit);
    const top = ticks[ticks.length - 1] ?? 1;
    const labels = ticks.map((v) => formatValue(v, this.unit));
    const x0 = this.left;
    const x1 = w - PAD.right;
    const y0 = PAD.top;
    const y1 = h - PAD.bottom;
    this.geo = { x0, x1, y0, y1, top, from, to, muted, text };
    const range = Math.max(1e-9, to - from);
    const X = (t: number) => x0 + ((t - from) / range) * (x1 - x0);
    const Y = (v: number) => y1 - (Math.min(Math.max(v, 0), top) / top) * (y1 - y0);

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
    const { x0, x1, y0, y1, top, from, to, muted, text } = geo;
    const range = Math.max(1e-9, to - from);
    const X = (t: number) => x0 + ((t - from) / range) * (x1 - x0);
    const Y = (v: number) => y1 - (Math.min(Math.max(v, 0), top) / top) * (y1 - y0);

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

    if (this.cursor === null || this.cursor < from || this.cursor > to) {
      this.tip.hidden = true;
      return;
    }
    const x = Math.round(X(this.cursor)) + 0.5;
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
      const p = nearest(line.points, this.cursor);
      if (p && Math.abs(p[0] - this.cursor) <= Math.max(this.span.step * 1.5, 3)) rows.push({ line, p });
    }
    for (const { line, p } of rows) {
      ctx.fillStyle = line.color;
      ctx.beginPath();
      ctx.arc(X(p[0]), Y(p[1]), 3, 0, Math.PI * 2);
      ctx.fill();
    }
    this.showTip(x, this.cursor, rows);
  }

  private showTip(x: number, t: number, rows: { line: Line; p: Point }[]) {
    if (!rows.length) {
      this.tip.hidden = true;
      return;
    }
    const head = document.createElement("div");
    head.className = "chart-tip-time";
    head.textContent = formatTime(rows[0]!.p[0] || t);
    const body: HTMLElement[] = [head];
    const sorted = [...rows].sort((a, b) => b.p[1] - a.p[1] || a.line.label.localeCompare(b.line.label));
    for (const { line, p } of sorted) {
      const row = document.createElement("div");
      row.className = "chart-tip-row";
      const swatch = document.createElement("i");
      swatch.style.background = line.color;
      const label = document.createElement("span");
      label.textContent = line.label;
      const value = document.createElement("b");
      value.textContent = formatValue(p[1], this.unit);
      row.append(swatch, label, value);
      if (this.span.step > 1 && p[3] > p[2]) {
        const band = document.createElement("small");
        band.textContent = `${formatValue(p[2], this.unit)} – ${formatValue(p[3], this.unit)}`;
        row.append(band);
      }
      body.push(row);
    }
    this.tip.replaceChildren(...body);
    this.tip.hidden = false;
    const width = this.tip.offsetWidth;
    const plotWidth = this.plot.clientWidth;
    this.tip.style.left = `${x + 14 + width > plotWidth ? Math.max(4, x - 14 - width) : x + 14}px`;
    this.tip.style.top = `${PAD.top}px`;
  }
}
