import { DatePicker } from "./useDatePicker.js";

export type RangeExpr = { from: string; to: string };
export type Refresh = "live" | "off" | "5" | "10" | "30" | "60" | "300";
export type Quick = { label: string; from: string; to: string };

export const QUICK: Quick[] = [
  { label: "Last 5 minutes", from: "now-5m", to: "now" },
  { label: "Last 15 minutes", from: "now-15m", to: "now" },
  { label: "Last 30 minutes", from: "now-30m", to: "now" },
  { label: "Last 1 hour", from: "now-1h", to: "now" },
  { label: "Last 3 hours", from: "now-3h", to: "now" },
  { label: "Last 6 hours", from: "now-6h", to: "now" },
  { label: "Last 12 hours", from: "now-12h", to: "now" },
  { label: "Last 24 hours", from: "now-24h", to: "now" },
  { label: "Last 2 days", from: "now-2d", to: "now" },
  { label: "Last 7 days", from: "now-7d", to: "now" },
  { label: "Last 30 days", from: "now-30d", to: "now" },
  { label: "Last 90 days", from: "now-90d", to: "now" },
  { label: "Last 6 months", from: "now-6M", to: "now" },
  { label: "Last 1 year", from: "now-1y", to: "now" },
  { label: "Today so far", from: "now/d", to: "now" },
  { label: "Today", from: "now/d", to: "now/d" },
  { label: "Yesterday", from: "now-1d/d", to: "now-1d/d" },
  { label: "Day before yesterday", from: "now-2d/d", to: "now-2d/d" },
  { label: "This week so far", from: "now/w", to: "now" },
  { label: "Previous week", from: "now-1w/w", to: "now-1w/w" },
  { label: "This month so far", from: "now/M", to: "now" },
  { label: "Previous month", from: "now-1M/M", to: "now-1M/M" },
];

export const REFRESH: { value: Refresh; label: string }[] = [
  { value: "live", label: "Live" },
  { value: "off", label: "Off" },
  { value: "5", label: "5s" },
  { value: "10", label: "10s" },
  { value: "30", label: "30s" },
  { value: "60", label: "1m" },
  { value: "300", label: "5m" },
];

const RELATIVE = /^now\s*((?:[+-]\s*\d+\s*[smhdwMy]\s*)*)(?:\/\s*([smhdwMy]))?$/i;
const TERM = /([+-])\s*(\d+)\s*([smhdwMy])/g;

function pad(n: number, width = 2): string {
  return String(n).padStart(width, "0");
}

function startOf(date: Date, unit: string): Date {
  const d = new Date(date);
  switch (unit) {
    case "s":
      d.setMilliseconds(0);
      break;
    case "m":
      d.setSeconds(0, 0);
      break;
    case "h":
      d.setMinutes(0, 0, 0);
      break;
    case "d":
      d.setHours(0, 0, 0, 0);
      break;
    case "w":
      d.setHours(0, 0, 0, 0);
      d.setDate(d.getDate() - ((d.getDay() + 6) % 7));
      break;
    case "M":
      d.setHours(0, 0, 0, 0);
      d.setDate(1);
      break;
    case "y":
      d.setHours(0, 0, 0, 0);
      d.setMonth(0, 1);
      break;
  }
  return d;
}

function add(date: Date, n: number, unit: string): Date {
  const d = new Date(date);
  switch (unit) {
    case "s":
      d.setSeconds(d.getSeconds() + n);
      break;
    case "m":
      d.setMinutes(d.getMinutes() + n);
      break;
    case "h":
      d.setHours(d.getHours() + n);
      break;
    case "d":
      d.setDate(d.getDate() + n);
      break;
    case "w":
      d.setDate(d.getDate() + n * 7);
      break;
    case "M":
      d.setMonth(d.getMonth() + n);
      break;
    case "y":
      d.setFullYear(d.getFullYear() + n);
      break;
  }
  return d;
}

export function formatAbs(t: number): string {
  const d = new Date(t * 1000);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export function resolve(expr: string, roundUp: boolean, now = new Date()): number | null {
  const text = expr.trim();
  if (!text) return null;
  const match = RELATIVE.exec(text);
  if (match) {
    let date = new Date(now);
    for (const term of (match[1] ?? "").matchAll(TERM)) {
      const n = Number(term[2]) * (term[1] === "-" ? -1 : 1);
      date = add(date, n, term[3]!);
    }
    const unit = match[2];
    if (unit) {
      const start = startOf(date, unit);
      date = roundUp ? new Date(add(start, 1, unit).getTime() - 1) : start;
    }
    return Math.floor(date.getTime() / 1000);
  }
  const absolute = /^(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}):(\d{2})(?::(\d{2}))?)?$/.exec(text);
  if (absolute) {
    const [, y, mo, d, h, mi, s] = absolute;
    const date = new Date(Number(y), Number(mo) - 1, Number(d), Number(h ?? 0), Number(mi ?? 0), Number(s ?? (roundUp && h === undefined ? 59 : 0)));
    if (roundUp && h === undefined) date.setHours(23, 59, 59, 0);
    return Number.isNaN(date.getTime()) ? null : Math.floor(date.getTime() / 1000);
  }
  if (/^\d{10}$/.test(text)) return Number(text);
  return null;
}

export function isNow(expr: string): boolean {
  return expr.trim().toLowerCase() === "now";
}

export function quickFor(range: RangeExpr): Quick | undefined {
  return QUICK.find((q) => q.from === range.from.trim() && q.to === range.to.trim());
}

export function describe(range: RangeExpr): string {
  return quickFor(range)?.label ?? `${range.from.trim()} to ${range.to.trim()}`;
}

export function relativeFor(length: number): RangeExpr {
  const units: [number, string][] = [
    [86400, "d"],
    [3600, "h"],
    [60, "m"],
  ];
  for (const [size, unit] of units) {
    if (length % size === 0) return { from: `now-${length / size}${unit}`, to: "now" };
  }
  return { from: `now-${Math.round(length)}s`, to: "now" };
}

function zone(): string {
  const offset = -new Date().getTimezoneOffset();
  const sign = offset < 0 ? "-" : "+";
  const abs = Math.abs(offset);
  const name = Intl.DateTimeFormat().resolvedOptions().timeZone;
  return `${name ? `${name}, ` : ""}UTC${sign}${pad(Math.floor(abs / 60))}:${pad(abs % 60)}`;
}

const ICONS = {
  clock: '<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>',
  chevron: '<path d="m6 9 6 6 6-6"/>',
  left: '<path d="m15 18-6-6 6-6"/>',
  right: '<path d="m9 18 6-6-6-6"/>',
  zoom: '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3M8 11h6"/>',
  calendar: '<path d="M8 2v4M16 2v4"/><rect width="18" height="18" x="3" y="4" rx="2"/><path d="M3 10h18"/>',
  refresh: '<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>',
  check: '<path d="M20 6 9 17l-5-5"/>',
};

function svg(name: keyof typeof ICONS, size = 16): string {
  return `<svg class="tp-icon" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ICONS[name]}</svg>`;
}

const RECENT = "senn:ranges";

function recent(): RangeExpr[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(RECENT) ?? "[]") as RangeExpr[];
    return parsed.filter((r) => typeof r.from === "string" && typeof r.to === "string").slice(0, 4);
  } catch {
    return [];
  }
}

function remember(range: RangeExpr) {
  if (quickFor(range)) return;
  const list = [range, ...recent().filter((r) => r.from !== range.from || r.to !== range.to)].slice(0, 4);
  try {
    localStorage.setItem(RECENT, JSON.stringify(list));
  } catch {
    return;
  }
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, className?: string, html?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (html !== undefined) node.innerHTML = html;
  return node;
}

export class TimePicker {
  readonly el: HTMLElement;
  onRange: (range: RangeExpr) => void = () => {};
  onRefresh: (mode: Refresh) => void = () => {};
  onNow: () => void = () => {};
  onShift: (dir: number) => void = () => {};
  onZoomOut: () => void = () => {};
  private range: RangeExpr;
  private refresh: Refresh;
  private pickers: DatePicker[] = [];
  private button: HTMLButtonElement;
  private label: HTMLElement;
  private pop: HTMLElement;
  private from: HTMLInputElement;
  private to: HTMLInputElement;
  private fromNote: HTMLElement;
  private toNote: HTMLElement;
  private apply: HTMLButtonElement;
  private recentList: HTMLElement;
  private quickList: HTMLElement;
  private search: HTMLInputElement;
  private refreshButton: HTMLButtonElement;
  private refreshMenu: HTMLElement;
  private refreshLabel: HTMLElement;
  private outside = (event: Event) => {
    const target = event.target as Node;
    for (const dp of this.pickers) {
      if (dp.isOpen && !dp.el.contains(target) && !(target as HTMLElement).closest?.(".tp-cal")) dp.close();
    }
    if (!this.pop.hidden && !this.pop.contains(target) && !this.button.contains(target)) this.close();
    if (!this.refreshMenu.hidden && !this.refreshMenu.contains(target) && !this.refreshButton.contains(target)) this.refreshMenu.hidden = true;
  };
  private escape = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return;
    this.close();
    this.refreshMenu.hidden = true;
  };

  constructor(range: RangeExpr, refresh: Refresh) {
    this.range = range;
    this.refresh = refresh;
    this.el = el("div", "tp");

    const picker = el("div", "tp-picker");
    this.button = el("button", "tp-button");
    this.button.type = "button";
    this.button.setAttribute("aria-haspopup", "dialog");
    this.label = el("span", "tp-label");
    this.button.innerHTML = svg("clock");
    this.button.append(this.label);
    this.button.insertAdjacentHTML("beforeend", svg("chevron", 14));
    this.pop = el("div", "tp-pop");
    this.pop.hidden = true;
    picker.append(this.button, this.pop);

    const abs = el("form", "tp-abs");
    abs.noValidate = true;
    abs.append(el("h4", undefined, "Absolute time range"));
    const field = (name: string, title: string) => {
      const wrap = el("div", "tp-field");
      const label = el("label", undefined, title);
      const row = el("div", "tp-input");
      const input = el("input");
      input.type = "text";
      input.name = name;
      input.spellcheck = false;
      input.autocomplete = "off";
      input.placeholder = name === "from" ? "now-6h" : "now";
      const calendar = el("button", "tp-cal", svg("calendar"));
      calendar.type = "button";
      calendar.setAttribute("aria-label", `Pick ${name} date`);
      const dp = new DatePicker();
      this.pickers.push(dp);
      calendar.addEventListener("click", () => {
        const wasOpen = dp.isOpen;
        this.closePickers();
        if (!wasOpen) dp.open(resolve(input.value, name === "to"));
      });
      dp.onPick = (t) => {
        input.value = formatAbs(t);
        this.validate();
      };
      row.append(input, calendar);
      const note = el("small", "tp-note");
      wrap.append(label, row, note, dp.el);
      return { wrap, input, note };
    };
    const from = field("from", "From");
    const to = field("to", "To");
    this.from = from.input;
    this.to = to.input;
    this.fromNote = from.note;
    this.toNote = to.note;
    this.apply = el("button", "button primary tp-apply", "Apply time range");
    this.apply.type = "submit";
    this.recentList = el("div", "tp-recent");
    const foot = el("p", "tp-zone muted", `Browser time <b>${zone()}</b>`);
    abs.append(from.wrap, to.wrap, this.apply, this.recentList, foot);
    abs.addEventListener("submit", (event) => {
      event.preventDefault();
      if (!this.validate()) return;
      this.commit({ from: this.from.value.trim(), to: this.to.value.trim() });
    });
    this.from.addEventListener("input", () => this.validate());
    this.to.addEventListener("input", () => this.validate());

    const quick = el("div", "tp-quick");
    this.search = el("input");
    this.search.type = "search";
    this.search.placeholder = "Search quick ranges";
    this.search.setAttribute("aria-label", "Search quick ranges");
    this.quickList = el("div", "tp-list");
    this.quickList.setAttribute("role", "listbox");
    quick.append(this.search, this.quickList);
    this.search.addEventListener("input", () => this.renderQuick());
    this.pop.append(abs, quick);

    this.button.addEventListener("click", () => (this.pop.hidden ? this.open() : this.close()));

    const nav = el("div", "tp-nav");
    const mk = (icon: keyof typeof ICONS, title: string, fn: () => void) => {
      const b = el("button", "tp-icon-button", svg(icon));
      b.type = "button";
      b.title = title;
      b.setAttribute("aria-label", title);
      b.addEventListener("click", fn);
      return b;
    };
    nav.append(
      mk("left", "Shift back", () => this.onShift(-1)),
      mk("right", "Shift forward", () => this.onShift(1)),
      mk("zoom", "Zoom out", () => this.onZoomOut()),
    );

    const refreshWrap = el("div", "tp-refresh");
    const now = mk("refresh", "Refresh now", () => this.onNow());
    this.refreshButton = el("button", "tp-refresh-button");
    this.refreshButton.type = "button";
    this.refreshButton.setAttribute("aria-haspopup", "menu");
    this.refreshLabel = el("span");
    this.refreshButton.append(this.refreshLabel);
    this.refreshButton.insertAdjacentHTML("beforeend", svg("chevron", 14));
    this.refreshMenu = el("div", "tp-menu");
    this.refreshMenu.hidden = true;
    this.refreshMenu.setAttribute("role", "menu");
    this.refreshButton.addEventListener("click", () => {
      this.refreshMenu.hidden = !this.refreshMenu.hidden;
      this.close();
    });
    refreshWrap.append(now, this.refreshButton, this.refreshMenu);

    this.el.append(picker, nav, refreshWrap);
    document.addEventListener("pointerdown", this.outside);
    document.addEventListener("keydown", this.escape);
    this.sync();
  }

  destroy() {
    document.removeEventListener("pointerdown", this.outside);
    document.removeEventListener("keydown", this.escape);
  }

  setRange(range: RangeExpr) {
    this.range = range;
    this.sync();
  }

  setRefresh(mode: Refresh) {
    this.refresh = mode;
    this.sync();
  }

  private commit(range: RangeExpr) {
    remember(range);
    this.close();
    this.onRange(range);
  }

  private open() {
    this.from.value = this.range.from;
    this.to.value = this.range.to;
    this.search.value = "";
    this.refreshMenu.hidden = true;
    this.validate();
    this.renderQuick();
    this.renderRecent();
    this.pop.hidden = false;
    this.button.setAttribute("aria-expanded", "true");
    const rect = this.pop.getBoundingClientRect();
    this.pop.classList.toggle("flip", rect.right > window.innerWidth - 8 && rect.left - (rect.width - this.button.offsetWidth) > 8);
  }

  private closePickers() {
    for (const dp of this.pickers) dp.close();
  }

  private close() {
    this.closePickers();
    this.pop.hidden = true;
    this.button.setAttribute("aria-expanded", "false");
  }

  private validate(): boolean {
    const a = resolve(this.from.value, false);
    const b = resolve(this.to.value, true);
    const bad = (input: HTMLInputElement, note: HTMLElement, message: string | null) => {
      input.classList.toggle("bad", message !== null);
      note.textContent = message ?? "";
    };
    bad(this.from, this.fromNote, a === null ? "Use now-6h, now/d or 2026-11-24 12:00:00" : null);
    bad(this.to, this.toNote, b === null ? "Use now, now-1h or 2026-11-24 12:00:00" : a !== null && b <= a ? "To must be after from" : null);
    const ok = a !== null && b !== null && b > a;
    this.apply.disabled = !ok;
    return ok;
  }

  private renderQuick() {
    const needle = this.search.value.trim().toLowerCase();
    this.quickList.replaceChildren();
    const active = quickFor(this.range);
    for (const q of QUICK) {
      if (needle && !q.label.toLowerCase().includes(needle)) continue;
      const b = el("button", "tp-item", `<span>${q.label}</span>${q === active ? svg("check", 14) : ""}`);
      b.type = "button";
      b.setAttribute("role", "option");
      b.classList.toggle("active", q === active);
      b.addEventListener("click", () => this.commit({ from: q.from, to: q.to }));
      this.quickList.append(b);
    }
    if (!this.quickList.childElementCount) this.quickList.append(el("p", "tp-empty muted", "No ranges match"));
  }

  private renderRecent() {
    this.recentList.replaceChildren();
    const list = recent();
    if (!list.length) return;
    this.recentList.append(el("h4", undefined, "Recently used ranges"));
    for (const r of list) {
      const b = el("button", "tp-item");
      b.type = "button";
      b.textContent = `${r.from} to ${r.to}`;
      b.addEventListener("click", () => this.commit(r));
      this.recentList.append(b);
    }
  }

  private sync() {
    this.label.textContent = describe(this.range);
    const a = resolve(this.range.from, false);
    const b = resolve(this.range.to, true);
    this.button.title = a !== null && b !== null ? `${formatAbs(a)} to ${formatAbs(b)}` : "";
    const current = REFRESH.find((r) => r.value === this.refresh) ?? REFRESH[0]!;
    this.refreshLabel.textContent = current.label;
    this.refreshButton.classList.toggle("live", this.refresh === "live" && isNow(this.range.to));
    this.refreshMenu.replaceChildren();
    for (const option of REFRESH) {
      const item = el("button", "tp-item", `<span>${option.label}</span>${option.value === this.refresh ? svg("check", 14) : ""}`);
      item.type = "button";
      item.setAttribute("role", "menuitemradio");
      item.classList.toggle("active", option.value === this.refresh);
      item.addEventListener("click", () => {
        this.refreshMenu.hidden = true;
        this.refresh = option.value;
        this.sync();
        this.onRefresh(option.value);
      });
      this.refreshMenu.append(item);
    }
  }
}

export {};
