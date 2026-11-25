import { escapeHtml } from "./useStream.js";

type Host = {
  cpu: number;
  cores: number;
  load: [number, number, number];
  mem_total: number;
  mem_used: number;
  swap_total: number;
  swap_used: number;
  disk_total: number;
  disk_used: number;
  net_rx: number;
  net_tx: number;
  io_read: number;
  io_write: number;
  uptime: number;
  extra?: {
    procs_total: number;
    procs_running: number;
    tcp_inuse: number;
    io_busy: number;
    temp?: number | null;
    pressure_cpu?: number | null;
  } | null;
};

function bytes(n: number): string {
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${i === 0 || v >= 100 ? v.toFixed(0) : v.toFixed(1)} ${units[i]}`;
}

function bits(perSec: number): string {
  const b = perSec * 8;
  const [v, unit] = b >= 1e9 ? [b / 1e9, "Gbps"] : b >= 1e6 ? [b / 1e6, "Mbps"] : b >= 1e3 ? [b / 1e3, "Kbps"] : [b, "bps"];
  return `${unit === "bps" || v >= 100 ? v.toFixed(0) : v.toFixed(1)} ${unit}`;
}

function duration(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d === 0 && h === 0) return `${m}m`;
  if (d === 0) return `${h}h ${m}m`;
  return `${d}d ${h}h`;
}

const pct = (used: number, total: number) => (total ? (used / total) * 100 : 0);
const tone = (p: number) => (p >= 90 ? "bad" : p >= 70 ? "warn" : "ok");

function set(card: Element, key: string, html: string, raw = false) {
  for (const el of card.querySelectorAll<HTMLElement>(`[data-stat="${key}"]`)) {
    if (raw) {
      if (el.innerHTML !== html) el.innerHTML = html;
    } else if (el.textContent !== html) el.textContent = html;
  }
}

function meter(card: Element, key: string, p: number) {
  const el = card.querySelector<HTMLElement>(`[data-meter="${key}"]`);
  if (!el) return;
  el.style.width = `${Math.min(100, p).toFixed(1)}%`;
  el.className = tone(p);
}

function apply(card: Element, h: Host) {
  const mem = pct(h.mem_used, h.mem_total);
  const disk = pct(h.disk_used, h.disk_total);
  set(card, "cpu", `${h.cpu.toFixed(1)}%`);
  meter(card, "cpu", h.cpu);
  set(card, "cpu-sub", `${h.cores} cores · load ${h.load[0].toFixed(2)}`);
  set(card, "mem", `${mem.toFixed(0)}%`);
  meter(card, "mem", mem);
  set(card, "mem-sub", `${bytes(h.mem_used)} / ${bytes(h.mem_total)}`);
  set(card, "disk", `${disk.toFixed(0)}%`);
  meter(card, "disk", disk);
  set(card, "disk-sub", `${bytes(h.disk_used)} / ${bytes(h.disk_total)}`);
  set(card, "uptime", duration(h.uptime));
  set(card, "load", `${h.load[0].toFixed(2)} <small>${h.load[1].toFixed(2)} · ${h.load[2].toFixed(2)}</small>`, true);
  set(card, "swap", `${escapeHtml(bytes(h.swap_used))} <small>of ${escapeHtml(bytes(h.swap_total))}</small>`, true);
  set(card, "rx", bits(h.net_rx));
  set(card, "tx", bits(h.net_tx));
  set(card, "read", `${bytes(h.io_read)}/s`);
  set(card, "write", `${bytes(h.io_write)}/s`);
  const x = h.extra;
  if (x) {
    set(card, "procs", `${x.procs_total} <small>${x.procs_running} running</small>`, true);
    set(card, "tcp", String(x.tcp_inuse));
    set(card, "busy", `${x.io_busy.toFixed(0)}%`);
    if (x.temp != null) set(card, "temp", `${x.temp.toFixed(0)} °C`);
    if (x.pressure_cpu != null) set(card, "pressure", `${x.pressure_cpu.toFixed(1)}%`);
  }
}

window.onLive((message) => {
  if (message.t !== "hub" || message.kind !== "stats") return;
  const host = (message.data as { host?: Host | null } | null)?.host;
  if (!host || typeof message.agent !== "string") return;
  for (const card of document.querySelectorAll(`[data-host-card="${CSS.escape(message.agent)}"]`)) apply(card, host);
});

export {};
