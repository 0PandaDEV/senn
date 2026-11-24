import { Chart } from "./useChart.js";
import type { Line, Point, Span, Unit } from "./useChart.js";
import { apiError } from "./useStream.js";
import { TimePicker, formatAbs, isNow, relativeFor, resolve } from "./useTimeRange.js";
import type { RangeExpr, Refresh } from "./useTimeRange.js";

type Def = { id: string; metric: string; label: string; scale?: number; dashed?: boolean; by?: string };
type ChartDef = { group: string; title: string; unit: Unit; yMax?: number; hint?: string; defs: Def[] };
type Scope = { stack?: string; container?: string };
type Sample = {
  agent: string;
  ts: number;
  host: Record<string, number>;
  containers: { container: string; stack: string | null; service: string | null; values: Record<string, number> }[];
};
type Series = { labels: Record<string, string>; points: Point[] };
type Reply = { step: number; series: Series[] };

const PALETTE = ["#7aa7ff", "#46c27f", "#e3a857", "#d98fb4", "#6ccbd9", "#c58ae0", "#b5c46c", "#f0706d"];
const STEPS = [1, 2, 3, 5, 6, 10, 15, 20, 30, 60, 120, 180, 300, 600, 900, 1800, 3600, 7200, 10800, 21600, 43200, 86400];
const BUDGET = 3600;
const KEY = "senn:range";

const d = (id: string, metric: string, label: string, extra: Partial<Def> = {}): Def => ({ id, metric, label, ...extra });
const host = (name: string) => `senn_host_${name}`;
const ctr = (name: string) => `senn_container_${name}`;
const chart = (group: string, title: string, unit: Unit, defs: Def[], yMax?: number, hint?: string): ChartDef => ({ group, title, unit, yMax, hint, defs });
const PSI = "Pressure stall info is off in this kernel. Boot with psi=1 to enable it.";
const by = (label: string): Partial<Def> => ({ by: label });
const bits = (extra: Partial<Def> = {}): Partial<Def> => ({ scale: 8, ...extra });

const SETS: Record<string, ChartDef[]> = {
  host: [
    chart("CPU", "CPU", "percent", [d("cpu", host("cpu_percent"), "CPU")], 100),
    chart("CPU", "CPU breakdown", "percent", [
      d("user", host("cpu_user_percent"), "User"),
      d("system", host("cpu_system_percent"), "System"),
      d("iowait", host("cpu_iowait_percent"), "I/O wait"),
      d("steal", host("cpu_steal_percent"), "Steal"),
      d("irq", host("cpu_irq_percent"), "IRQ"),
    ]),
    chart("CPU", "Load average", "number", [d("l1", host("load1"), "1 min"), d("l5", host("load5"), "5 min"), d("l15", host("load15"), "15 min")]),
    chart("CPU", "Pressure", "percent", [
      d("pc", host("pressure_cpu_percent"), "CPU"),
      d("pm", host("pressure_memory_percent"), "Memory"),
      d("pmf", host("pressure_memory_full_percent"), "Memory (full)"),
      d("pi", host("pressure_io_percent"), "I/O"),
      d("pif", host("pressure_io_full_percent"), "I/O (full)"),
    ], undefined, PSI),
    chart("Memory", "Memory", "bytes", [
      d("used", host("mem_used_bytes"), "Used"),
      d("cached", host("mem_cached_bytes"), "Cached"),
      d("buffers", host("mem_buffers_bytes"), "Buffers"),
      d("free", host("mem_free_bytes"), "Free"),
      d("total", host("mem_total_bytes"), "Total", { dashed: true }),
    ]),
    chart("Memory", "Swap", "bytes", [d("used", host("swap_used_bytes"), "Used"), d("total", host("swap_total_bytes"), "Total", { dashed: true })]),
    chart("Memory", "Swap activity", "rate", [d("in", host("swap_in_bytes_per_second"), "Swap in"), d("out", host("swap_out_bytes_per_second"), "Swap out")]),
    chart("Disk", "Disk usage", "bytes", [d("used", host("disk_used_bytes"), "Used"), d("total", host("disk_total_bytes"), "Total", { dashed: true })]),
    chart("Disk", "Disk throughput", "rate", [d("read", host("io_read_bytes_per_second"), "Read"), d("write", host("io_write_bytes_per_second"), "Write")]),
    chart("Disk", "Disk operations", "ops", [d("read", host("io_read_ops_per_second"), "Reads"), d("write", host("io_write_ops_per_second"), "Writes")]),
    chart("Disk", "Disk busy", "percent", [d("busy", host("io_busy_percent"), "Busiest disk")], 100),
    chart("Network", "Network traffic", "bits", [d("rx", host("net_rx_bytes_per_second"), "In", bits()), d("tx", host("net_tx_bytes_per_second"), "Out", bits())]),
    chart("Network", "Network packets", "ops", [d("rx", host("net_rx_packets_per_second"), "In"), d("tx", host("net_tx_packets_per_second"), "Out")]),
    chart("Network", "Network errors and drops", "ops", [d("rx", host("net_rx_errors_per_second"), "In"), d("tx", host("net_tx_errors_per_second"), "Out")]),
    chart("System", "Processes", "number", [
      d("total", host("processes_total"), "Total"),
      d("running", host("processes_running"), "Running"),
      d("blocked", host("processes_blocked"), "Blocked"),
    ]),
    chart("System", "TCP connections", "number", [d("tcp", host("tcp_connections"), "In use")]),
    chart("System", "Open files", "number", [d("files", host("open_files"), "File handles")]),
    chart("System", "Context switches and interrupts", "ops", [d("ctx", host("context_switches_per_second"), "Context switches"), d("irq", host("interrupts_per_second"), "Interrupts")]),
    chart("System", "Temperature", "celsius", [d("temp", host("temperature_celsius"), "Hottest sensor")], undefined, "This host exposes no temperature sensor."),
    chart("System", "Uptime", "duration", [d("up", host("uptime_seconds"), "Uptime")]),
    chart("Docker", "Containers", "count", [d("running", ctr("running"), "Running", by("sum")), d("stopped", ctr("running"), "Not running", by("stopped"))]),
    chart("Docker", "Restarting containers", "count", [d("restarting", ctr("restarting"), "Restarting", by("sum"))]),
    chart("Docker", "Docker objects", "count", [
      d("images", host("docker_images"), "Images"),
      d("volumes", host("docker_volumes"), "Volumes"),
      d("networks", host("docker_networks"), "Networks"),
    ]),
    chart("Docker", "Image storage", "bytes", [d("size", host("docker_images_bytes"), "Images")]),
    chart("Per container", "Container CPU", "percent", [d("cpu", ctr("cpu_percent"), "CPU", by("container"))]),
    chart("Per container", "Container memory", "bytes", [d("mem", ctr("mem_bytes"), "Memory", by("container"))]),
    chart("Per container", "Container memory of limit", "percent", [d("memp", ctr("mem_percent"), "Memory", by("container"))]),
    chart("Per container", "Container network in", "bits", [d("rx", ctr("net_rx_bytes_per_second"), "In", bits(by("container")))]),
    chart("Per container", "Container network out", "bits", [d("tx", ctr("net_tx_bytes_per_second"), "Out", bits(by("container")))]),
    chart("Per container", "Container disk read", "rate", [d("read", ctr("io_read_bytes_per_second"), "Read", by("container"))]),
    chart("Per container", "Container disk write", "rate", [d("write", ctr("io_write_bytes_per_second"), "Write", by("container"))]),
    chart("Per container", "Container processes", "number", [d("pids", ctr("pids"), "Processes", by("container"))]),
  ],
  stack: [
    chart("Resources", "CPU", "percent", [d("cpu", ctr("cpu_percent"), "CPU", by("service"))]),
    chart("Resources", "CPU throttling", "ops", [d("thr", ctr("cpu_throttled_per_second"), "Throttled periods", by("service"))]),
    chart("Resources", "Memory", "bytes", [d("mem", ctr("mem_bytes"), "Memory", by("service"))]),
    chart("Resources", "Processes", "number", [d("pids", ctr("pids"), "Processes", by("service"))]),
    chart("Network", "Network traffic", "bits", [d("rx", ctr("net_rx_bytes_per_second"), "In", bits(by("sum"))), d("tx", ctr("net_tx_bytes_per_second"), "Out", bits(by("sum")))]),
    chart("Network", "Network packets", "ops", [d("rx", ctr("net_rx_packets_per_second"), "In", by("sum")), d("tx", ctr("net_tx_packets_per_second"), "Out", by("sum"))]),
    chart("Disk", "Disk throughput", "rate", [d("read", ctr("io_read_bytes_per_second"), "Read", by("sum")), d("write", ctr("io_write_bytes_per_second"), "Write", by("sum"))]),
    chart("Disk", "Disk operations", "ops", [d("read", ctr("io_read_ops_per_second"), "Reads", by("sum")), d("write", ctr("io_write_ops_per_second"), "Writes", by("sum"))]),
    chart("State", "Containers", "count", [d("running", ctr("running"), "Running", by("sum")), d("stopped", ctr("running"), "Not running", by("stopped"))]),
    chart("State", "Health checks", "count", [d("ok", ctr("healthy"), "Healthy", by("sum")), d("bad", ctr("healthy"), "Unhealthy", by("stopped"))]),
    chart("State", "Restarting containers", "count", [d("restarting", ctr("restarting"), "Restarting", by("sum"))]),
  ],
  container: [
    chart("Resources", "CPU", "percent", [d("cpu", ctr("cpu_percent"), "CPU")]),
    chart("Resources", "CPU throttling", "ops", [d("thr", ctr("cpu_throttled_per_second"), "Throttled periods")]),
    chart("Resources", "Memory", "bytes", [d("mem", ctr("mem_bytes"), "Memory"), d("limit", ctr("mem_limit_bytes"), "Limit", { dashed: true })]),
    chart("Resources", "Memory of limit", "percent", [d("memp", ctr("mem_percent"), "Memory")], 100),
    chart("Resources", "Processes", "number", [d("pids", ctr("pids"), "Processes")]),
    chart("Network", "Network traffic", "bits", [d("rx", ctr("net_rx_bytes_per_second"), "In", bits()), d("tx", ctr("net_tx_bytes_per_second"), "Out", bits())]),
    chart("Network", "Network packets", "ops", [d("rx", ctr("net_rx_packets_per_second"), "In"), d("tx", ctr("net_tx_packets_per_second"), "Out")]),
    chart("Disk", "Disk throughput", "rate", [d("read", ctr("io_read_bytes_per_second"), "Read"), d("write", ctr("io_write_bytes_per_second"), "Write")]),
    chart("Disk", "Disk operations", "ops", [d("read", ctr("io_read_ops_per_second"), "Reads"), d("write", ctr("io_write_ops_per_second"), "Writes")]),
    chart("State", "State", "count", [
      d("running", ctr("running"), "Running"),
      d("restarting", ctr("restarting"), "Restarting"),
      d("healthy", ctr("healthy"), "Healthy"),
    ]),
  ],
};

type Slot = { chart: Chart; def: ChartDef; lines: Map<string, Line>; order: string[]; visible: boolean; onScreen: boolean; dirty: boolean; loaded: boolean; blank: boolean };

function seconds(): number {
  return Date.now() / 1000;
}

function pad(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

function sampleValues(def: Def, scope: Scope, sample: Sample): Map<string, number> {
  const out = new Map<string, number>();
  if (!def.metric.startsWith("senn_container")) {
    const v = sample.host[def.metric];
    if (v !== undefined) out.set("", v);
    return out;
  }
  const rows = sample.containers.filter((c) => (!scope.stack || c.stack === scope.stack) && (!scope.container || c.container === scope.container));
  const value = (c: Sample["containers"][number]) => c.values[def.metric];
  if (!def.by) {
    const v = rows[0] ? value(rows[0]) : undefined;
    if (v !== undefined) out.set("", v);
  } else if (def.by === "sum") {
    const found = rows.map(value).filter((v): v is number => v !== undefined);
    if (found.length) out.set("", found.reduce((a, b) => a + b, 0));
  } else if (def.by === "count") {
    out.set("", rows.length);
  } else if (def.by === "stopped") {
    out.set("", rows.length - rows.reduce((a, c) => a + (value(c) ?? 0), 0));
  } else {
    for (const c of rows) {
      const key = (c[def.by as "stack" | "service" | "container"] as string | null) ?? "";
      const v = value(c);
      if (v !== undefined) out.set(key, (out.get(key) ?? 0) + v);
    }
  }
  return out;
}

const DEFAULT: RangeExpr = { from: "now-1h", to: "now" };
const MODES: Refresh[] = ["live", "off", "5", "10", "30", "60", "300"];

class Panel {
  private slots: Slot[] = [];
  private range: RangeExpr = DEFAULT;
  private refresh: Refresh = "live";
  private span: Span = { from: 0, to: 1, step: 1 };
  private abort: AbortController | null = null;
  private timer = 0;
  private dirty = false;
  private appended = 0;
  private loadedAt = 0;
  private query: { from: number; to: number; step: number; signal: AbortSignal } | null = null;
  private owners = new WeakMap<Element, Slot>();
  private screen = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      const slot = this.owners.get(entry.target);
      if (slot) slot.onScreen = entry.isIntersecting;
    }
  });
  private observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        const slot = this.owners.get(entry.target);
        if (!slot) continue;
        slot.visible = entry.isIntersecting;
        if (slot.visible && slot.dirty && this.query) void this.fill(slot);
      }
    },
    { rootMargin: "300px 0px" },
  );
  private picker!: TimePicker;
  private scope: Scope;
  private handler: (m: { t: string; [key: string]: unknown }) => void;
  private onVisible = () => {
    if (!document.hidden && this.isLive() && this.refresh !== "off") void this.load();
  };

  constructor(
    readonly el: HTMLElement,
    private kind: string,
    private host: string,
    name: string,
  ) {
    this.scope = kind === "stack" ? { stack: name } : kind === "container" ? { container: name } : {};
    const saved = this.restore();
    this.range = saved.range;
    this.refresh = saved.refresh;
    el.classList.add("metrics");
    this.build();
    this.handler = (m) => {
      if (m.t === "sample") this.append(m as unknown as Sample);
      else if (m.t === "open" && this.isLive() && this.refresh !== "off") void this.load();
    };
    window.onLive((m) => this.handler(m));
    window.liveSub(host);
    document.addEventListener("visibilitychange", this.onVisible);
    void this.load();
    this.timer = window.setInterval(() => this.tick(), 1000);
  }

  destroy() {
    this.abort?.abort();
    this.observer.disconnect();
    this.screen.disconnect();
    clearInterval(this.timer);
    document.removeEventListener("visibilitychange", this.onVisible);
    this.picker.destroy();
    this.handler = () => {};
    window.liveSub(null);
  }

  visible(): boolean {
    return this.el.offsetParent !== null;
  }

  revive() {
    if (this.dirty && this.visible()) void this.load();
  }

  private restore(): { range: RangeExpr; refresh: Refresh } {
    const valid = (r: RangeExpr) => resolve(r.from, false) !== null && resolve(r.to, true) !== null;
    const mode = (v: unknown): Refresh => (MODES.includes(v as Refresh) ? (v as Refresh) : "live");
    const params = new URL(location.href).searchParams;
    const from = params.get("from");
    const to = params.get("to");
    if (from && to && valid({ from, to })) return { range: { from, to }, refresh: mode(params.get("refresh")) };
    try {
      const raw = sessionStorage.getItem(KEY);
      if (raw) {
        const parsed = JSON.parse(raw) as { range?: RangeExpr; refresh?: string };
        if (parsed.range && valid(parsed.range)) return { range: parsed.range, refresh: mode(parsed.refresh) };
      }
    } catch {
      return { range: DEFAULT, refresh: "live" };
    }
    return { range: DEFAULT, refresh: "live" };
  }

  private remember() {
    try {
      sessionStorage.setItem(KEY, JSON.stringify({ range: this.range, refresh: this.refresh }));
    } catch {
      return;
    }
    if (!this.visible()) return;
    const url = new URL(location.href);
    url.searchParams.set("from", this.range.from);
    url.searchParams.set("to", this.range.to);
    url.searchParams.set("refresh", this.refresh);
    history.replaceState(history.state, "", url);
  }

  private isLive(): boolean {
    return isNow(this.range.to);
  }

  private window(): [number, number] {
    const now = new Date();
    const stamp = Math.floor(now.getTime() / 1000);
    const to = resolve(this.range.to, true, now) ?? stamp;
    const from = resolve(this.range.from, false, now) ?? to - 3600;
    return from < to ? [from, to] : [to - 60, to];
  }

  private fixed(from: number, to: number): RangeExpr {
    const now = seconds();
    if (to >= now - 1) return relativeFor(Math.max(1, Math.round(to - from)));
    return { from: formatAbs(from), to: formatAbs(to) };
  }

  private shift(dir: number) {
    const [from, to] = this.window();
    const half = (to - from) / 2;
    const end = Math.min(to + dir * half, seconds());
    this.set(this.fixed(end - (to - from), end));
  }

  private zoomOut() {
    const [from, to] = this.window();
    const length = to - from;
    const now = seconds();
    const mid = (from + to) / 2;
    const end = to > now ? to + length / 2 : Math.min(mid + length, now);
    this.set(this.fixed(end - length * 2, end));
  }

  private build() {
    this.el.replaceChildren();
    this.picker = new TimePicker(this.range, this.refresh);
    this.picker.onRange = (range) => this.set(range);
    this.picker.onShift = (dir) => this.shift(dir);
    this.picker.onZoomOut = () => this.zoomOut();
    this.picker.onNow = () => void this.load();
    this.picker.onRefresh = (mode) => {
      this.refresh = mode;
      this.remember();
      if (mode !== "off") void this.load();
    };
    const bar = document.createElement("div");
    bar.className = "metrics-bar";
    bar.append(this.picker.el);
    const grid = document.createElement("div");
    grid.className = "chart-grid";
    this.el.append(bar, grid);
    let group = "";
    for (const def of SETS[this.kind] ?? []) {
      if (def.group !== group) {
        group = def.group;
        const heading = document.createElement("h3");
        heading.className = "chart-group";
        heading.textContent = group;
        grid.append(heading);
      }
      const chart = new Chart(grid, { title: def.title, unit: def.unit, yMax: def.yMax });
      const slot: Slot = { chart, def, lines: new Map(), order: [], visible: false, onScreen: false, dirty: true, loaded: false, blank: false };
      this.observer.observe(chart.el);
      this.screen.observe(chart.el);
      this.owners.set(chart.el, slot);
      chart.onCursor = (t) => {
        for (const s of this.slots) if (s !== slot && (s.onScreen || t === null)) s.chart.setCursor(t);
      };
      chart.onZoom = (from, to) => this.set({ from: formatAbs(Math.floor(from)), to: formatAbs(Math.ceil(to)) });
      chart.onZoomOut = () => this.zoomOut();
      this.slots.push(slot);
    }
  }

  private set(range: RangeExpr) {
    this.range = range;
    this.picker.setRange(range);
    this.remember();
    void this.load();
  }

  private stepFor(from: number, to: number): number {
    return STEPS.find((s) => s >= (to - from) / BUDGET) ?? STEPS[STEPS.length - 1]!;
  }

  private tick() {
    if (!this.el.isConnected || document.hidden || !this.isLive() || this.refresh === "off") return;
    if (!this.visible()) {
      this.dirty = true;
      return;
    }
    const age = seconds() - this.loadedAt;
    const limit = this.refresh === "live" ? (this.span.step <= 5 ? 120 : Math.max(15, this.span.step)) : Number(this.refresh);
    if (age >= limit) void this.load();
  }

  private async load() {
    this.abort?.abort();
    const abort = new AbortController();
    this.abort = abort;
    this.dirty = false;
    const [from, to] = this.window();
    const step = this.stepFor(from, to);
    this.loadedAt = seconds();
    this.query = { from, to, step, signal: abort.signal };
    for (const slot of this.slots) {
      slot.dirty = true;
      if (!slot.lines.size) slot.chart.note("Loading");
    }
    await Promise.all(this.slots.filter((slot) => slot.visible).map((slot) => this.fill(slot)));
  }

  private async fill(slot: Slot) {
    const q = this.query;
    if (!q) return;
    slot.dirty = false;
    await this.loadSlot(slot, q.from, q.to, q.step, q.signal);
    slot.loaded = true;
  }

  private async fetchDef(def: Def, from: number, to: number, step: number, signal: AbortSignal): Promise<Reply> {
    const q = new URLSearchParams({ host: this.host, metric: def.metric, from: String(from), to: String(to), step: String(step) });
    if (def.metric.startsWith("senn_container")) {
      if (this.scope.stack) q.set("stack", this.scope.stack);
      if (this.scope.container) q.set("container", this.scope.container);
    }
    if (def.by) q.set("by", def.by);
    const response = await fetch(`/api/metrics/query?${q}`, { credentials: "same-origin", signal });
    if (!response.ok) throw new Error(await apiError(response));
    return (await response.json()) as Reply;
  }

  private async loadSlot(slot: Slot, from: number, to: number, step: number, signal: AbortSignal) {
    try {
      const replies = await Promise.all(slot.def.defs.map((d) => this.fetchDef(d, from, to, step, signal)));
      if (signal.aborted) return;
      const lines = new Map<string, Line>();
      const order: string[] = [];
      let used = step;
      slot.def.defs.forEach((def, index) => {
        const reply = replies[index]!;
        used = Math.max(used, reply.step);
        const scale = def.scale ?? 1;
        const grouped = def.by !== undefined && !["sum", "count", "stopped"].includes(def.by);
        const sorted = grouped
          ? [...reply.series].sort((a, b) => (a.labels[def.by!] ?? "").localeCompare(b.labels[def.by!] ?? ""))
          : reply.series.slice(0, 1);
        sorted.forEach((series, i) => {
          const key = grouped ? (series.labels[def.by!] ?? "") : "";
          const id = `${def.id}:${key}`;
          const points = series.points.map((p) => [p[0], p[1] * scale, p[2] * scale, p[3] * scale] as Point);
          const name = grouped ? key || "(none)" : def.label;
          lines.set(id, { id, label: name, color: PALETTE[(grouped ? i : index) % PALETTE.length]!, points, dashed: def.dashed });
          order.push(id);
        });
      });
      for (const id of order) {
        const previous = slot.lines.get(id);
        if (previous?.hidden) lines.get(id)!.hidden = true;
      }
      slot.lines = lines;
      slot.order = order;
      this.span = { from, to, step: used };
      const has = [...lines.values()].some((l) => l.points.length);
      slot.blank = !has;
      slot.chart.note(null);
      slot.chart.empty(has ? null : (slot.def.hint ?? "No data in this range"));
      slot.chart.setLines([...lines.values()], this.span);
    } catch (error) {
      if (signal.aborted) return;
      slot.lines = new Map();
      slot.chart.empty(null);
      slot.chart.setLines([], { from, to, step });
      slot.chart.note((error as Error).message);
    }
  }

  private append(sample: Sample) {
    if (!this.isLive() || this.refresh !== "live" || this.span.step > 5 || !this.visible() || sample.agent !== this.host) return;
    const t = sample.ts / 1000;
    const length = this.span.to - this.span.from;
    this.span = { from: t - length, to: t, step: this.span.step };
    this.appended++;
    for (const slot of this.slots) {
      if (!slot.loaded || slot.dirty) continue;
      slot.def.defs.forEach((def, index) => {
        const grouped = def.by !== undefined && !["sum", "count", "stopped"].includes(def.by);
        const scale = def.scale ?? 1;
        for (const [key, value] of sampleValues(def, this.scope, sample)) {
          const id = `${def.id}:${grouped ? key : ""}`;
          let line = slot.lines.get(id);
          if (!line) {
            line = { id, label: grouped ? key || "(none)" : def.label, color: PALETTE[(grouped ? slot.lines.size : index) % PALETTE.length]!, points: [], dashed: def.dashed };
            slot.lines.set(id, line);
            slot.order.push(id);
          }
          const v = value * scale;
          const last = line.points[line.points.length - 1];
          if (last && last[0] >= t) continue;
          line.points.push([t, v, v, v]);
        }
      });
      for (const line of slot.lines.values()) {
        let drop = 0;
        while (drop < line.points.length && line.points[drop]![0] < this.span.from - 5) drop++;
        if (drop) line.points.splice(0, drop);
      }
      if (slot.blank && [...slot.lines.values()].some((l) => l.points.length)) {
        slot.blank = false;
        slot.chart.empty(null);
        slot.chart.setLines([...slot.lines.values()], this.span);
      }
      slot.chart.setSpan(this.span);
      if (this.appended % 5 === 0) slot.chart.setLines([...slot.lines.values()], this.span);
    }
  }
}

const panels = new Set<Panel>();

let queued = false;

function scan() {
  if (queued) return;
  queued = true;
  requestAnimationFrame(() => {
    queued = false;
    run();
  });
}

function run() {
  for (const panel of [...panels]) {
    if (!panel.el.isConnected) {
      panel.destroy();
      panels.delete(panel);
    } else panel.revive();
  }
  for (const el of document.querySelectorAll<HTMLElement>("[data-metrics]:not([data-bound])")) {
    if (el.closest("[hidden]")) continue;
    el.dataset.bound = "1";
    panels.add(new Panel(el, el.dataset.metrics!, el.dataset.host!, el.dataset.name ?? ""));
  }
}

new MutationObserver(scan).observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ["hidden"] });
document.addEventListener("senn:tab", scan);
scan();

export {};
