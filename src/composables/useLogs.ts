import { escapeHtml, stripAnsi } from "./useStream.js";

const MAX_LINES = 5000;
const PALETTE = ["#6cc4a1", "#e0a458", "#7aa7e0", "#d98fb4", "#b5c46c", "#c58ae0", "#6ccbd9", "#e07a6c"];

function hue(label: string): string {
  let hash = 0;
  for (const ch of label) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  return PALETTE[hash % PALETTE.length] ?? "#999";
}

const ERR = /\b(ERROR|ERR|FATAL|PANIC|CRITICAL|CRIT|EMERG|EXCEPTION)\b|\[(error|err|fatal|crit|emerg|alert)\]|\blevel=(error|err|fatal|crit)|(^|\s)(error|panic|fatal):/;
const WARN = /\b(WARN|WARNING)\b|\[(warn|warning)\]|\blevel=warn|(^|\s)warning:/;

function level(text: string): string {
  if (ERR.test(text)) return "err";
  if (WARN.test(text)) return "warn";
  return "";
}

const LEVEL_KEYS = ["level", "lvl", "severity", "log.level"];
const MSG_KEYS = ["msg", "message", "event"];
const TIME_KEYS = ["ts", "time", "timestamp", "@timestamp", "t"];

function levelOf(value: string): string {
  const v = value.toLowerCase();
  if (/^(err|error|fatal|panic|crit|critical|alert|emerg)/.test(v)) return "err";
  if (/^warn/.test(v)) return "warn";
  return "";
}

function scalar(value: unknown): string {
  if (typeof value === "string") return value;
  return JSON.stringify(value);
}

function structured(text: string): { cls: string; html: string } | null {
  const trimmed = text.trim();
  if (!trimmed.startsWith("{") || !trimmed.endsWith("}")) return null;
  let data: Record<string, unknown>;
  try {
    const parsed = JSON.parse(trimmed);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return null;
    data = parsed as Record<string, unknown>;
  } catch {
    return null;
  }
  const take = (keys: string[]) => {
    for (const k of keys) {
      if (k in data && data[k] !== null && typeof data[k] !== "object") {
        const v = scalar(data[k]);
        delete data[k];
        return v;
      }
    }
    return "";
  };
  const lvl = take(LEVEL_KEYS);
  const msg = take(MSG_KEYS);
  take(TIME_KEYS);
  const name = typeof data.logger === "string" ? data.logger : "";
  if (name) delete data.logger;
  const cls = levelOf(lvl);
  let html = "";
  if (lvl) html += `<span class="lvl ${cls}">${escapeHtml(lvl.toLowerCase())}</span>`;
  if (name) html += `<span class="logger">${escapeHtml(name)}</span>`;
  if (msg) html += `<span class="msg">${highlight(msg)}</span>`;
  const rest = Object.entries(data)
    .map(([k, v]) => `<span class="kv"><i>${escapeHtml(k)}=</i>${highlight(scalar(v))}</span>`)
    .join(" ");
  if (rest) html += rest;
  return { cls, html };
}

const MONTH = "(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)";
const TOKENS: [string, string][] = [
  ["url", String.raw`\b(?:https?|wss?|ftp|tcp|udp|unix):\/\/[^\s"'<>\`)\]]+`],
  ["time", String.raw`\b\d{4}[-/]\d\d[-/]\d\d(?:[T ]\d\d:\d\d(?::\d\d(?:[.,]\d+)?)?(?:Z|[+-]\d\d:?\d\d)?)?\b|\b\d{1,2} ${MONTH} \d{4}(?: \d\d:\d\d:\d\d(?:\.\d+)?)?|\b${MONTH} +\d{1,2} \d\d:\d\d:\d\d\b|\b\d\d:\d\d:\d\d(?:[.,]\d+)?\b`],
  ["err", String.raw`\b(?:ERROR|ERR|FATAL|PANIC|CRITICAL|CRIT|EMERG|ALERT|EXCEPTION|FAIL(?:ED|URE)?)\b|\[(?:error|err|fatal|crit|emerg|alert)\]|\b(?:error|panic|fatal):|(?<=^|\t|\s{2})(?:error|fatal|panic)(?=\t|\s{2}|$)`],
  ["warn", String.raw`\b(?:WARN(?:ING)?)\b|\[(?:warn|warning)\]|\bwarning:|(?<=^|\t|\s{2})warn(?:ing)?(?=\t|\s{2}|$)`],
  ["info", String.raw`\b(?:INFO|NOTICE)\b|\[(?:info|notice)\]|(?<=^|\t|\s{2})(?:info|notice)(?=\t|\s{2}|$)`],
  ["debug", String.raw`\b(?:DEBUG|TRACE|VERBOSE)\b|\[(?:debug|trace)\]|(?<=^|\t|\s{2})(?:debug|trace)(?=\t|\s{2}|$)`],
  ["str", String.raw`"(?:[^"\\]|\\.)*"|\`[^\`]*\``],
  ["path", String.raw`(?<=^|[\s=(])(?:\.{0,2}\/[\w.@+-]+)+\/?`],
];
const PATTERN = new RegExp(TOKENS.map(([name, re]) => `(?<${name}>${re})`).join("|"), "g");

function highlight(text: string): string {
  let html = "";
  let last = 0;
  for (const match of text.matchAll(PATTERN)) {
    const at = match.index!;
    if (!match[0]) continue;
    const name = Object.entries(match.groups!).find(([, v]) => v !== undefined)?.[0] ?? "";
    html += escapeHtml(text.slice(last, at));
    const lead = name === "time" && /^(?:\d+:[A-Z]\s)?$/.test(text.slice(0, at));
    html += `<span class="hl-${name}${lead ? " lead" : ""}">${escapeHtml(match[0])}</span>`;
    last = at + match[0].length;
  }
  return html + escapeHtml(text.slice(last));
}

type Viewer = { stop(): void; el: HTMLElement };
const viewers = new Set<Viewer>();

function mount(el: HTMLElement) {
  el.dataset.bound = "1";
  const url = el.dataset.logs!;
  const multi = el.hasAttribute("data-multi");
  el.classList.add("logs");
  el.innerHTML = `
    <div class="logs-bar">
      <input type="search" class="logs-filter" placeholder="Filter" aria-label="Filter">
      <label class="check inline"><input type="checkbox" class="logs-follow" checked><span>Follow</span></label>
      <label class="check inline"><input type="checkbox" class="logs-time" checked><span>Time</span></label>
      <span class="logs-status muted"></span>
    </div>
    <div class="logs-body" tabindex="0"></div>`;
  const body = el.querySelector<HTMLElement>(".logs-body")!;
  const filter = el.querySelector<HTMLInputElement>(".logs-filter")!;
  const follow = el.querySelector<HTMLInputElement>(".logs-follow")!;
  const time = el.querySelector<HTMLInputElement>(".logs-time")!;
  const status = el.querySelector<HTMLElement>(".logs-status")!;
  let widest = 0;
  body.classList.toggle("hide-time", !time.checked);

  const source = new EventSource(url);
  status.textContent = "connecting";
  source.addEventListener("open", () => (status.textContent = "live"));
  source.addEventListener("line", (event) => {
    let text = stripAnsi((event as MessageEvent<string>).data);
    let label = "";
    if (multi && !/^\d{4}-\d\d-\d\dT/.test(text)) {
      const tab = text.indexOf("\t");
      if (tab !== -1) {
        label = text.slice(0, tab);
        text = text.slice(tab + 1);
      }
    }
    let stamp = "";
    const space = text.indexOf(" ");
    if (space > 20 && /^\d{4}-\d\d-\d\dT/.test(text)) {
      stamp = `${text.slice(0, 10)} ${text.slice(11, 19)}`;
      text = text.slice(space + 1);
    }
    if (label && label.length > widest && widest < 20) {
      widest = Math.min(20, label.length);
      body.style.setProperty("--src-w", `${widest}ch`);
    }
    const row = document.createElement("div");
    const parsed = structured(text);
    row.className = `log ${parsed ? parsed.cls : level(text)}`;
    row.innerHTML =
      `<span class="time">${stamp ? `<span class="date">${escapeHtml(stamp.slice(0, 11))}</span>${escapeHtml(stamp.slice(11))}` : ""}</span>` +
      (label ? `<span class="source" style="--src:${hue(label)}">${escapeHtml(label)}</span>` : "") +
      `<span class="message${parsed ? " json" : ""}">${parsed ? parsed.html : highlight(text)}</span>`;
    const needle = filter.value.trim().toLowerCase();
    if (needle && !row.textContent!.toLowerCase().includes(needle)) row.hidden = true;
    body.appendChild(row);
    while (body.childElementCount > MAX_LINES) body.firstElementChild?.remove();
    if (follow.checked) body.scrollTop = body.scrollHeight;
  });
  source.addEventListener("end", () => {
    status.textContent = "ended";
    source.close();
  });
  source.addEventListener("error", () => {
    status.textContent = source.readyState === EventSource.CLOSED ? "closed" : "reconnecting";
  });

  filter.addEventListener("input", () => {
    const needle = filter.value.trim().toLowerCase();
    for (const row of body.children) {
      (row as HTMLElement).hidden = !!needle && !row.textContent!.toLowerCase().includes(needle);
    }
  });
  time.addEventListener("change", () => body.classList.toggle("hide-time", !time.checked));
  body.addEventListener("scroll", () => {
    if (body.scrollTop + body.clientHeight < body.scrollHeight - 40) follow.checked = false;
  });

  viewers.add({
    el,
    stop() {
      source.close();
    },
  });
}

function scan() {
  for (const v of [...viewers]) {
    if (!v.el.isConnected) {
      v.stop();
      viewers.delete(v);
    }
  }
  for (const el of document.querySelectorAll<HTMLElement>("[data-logs]:not([data-bound])")) {
    if (!el.closest("[hidden]")) mount(el);
  }
}

new MutationObserver(scan).observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ["hidden"] });
document.addEventListener("senn:tab", scan);
scan();

export {};
