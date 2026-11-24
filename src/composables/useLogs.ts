import { escapeHtml, stripAnsi } from "./useStream.js";

const MAX_LINES = 5000;
const PALETTE = ["#6cc4a1", "#e0a458", "#7aa7e0", "#d98fb4", "#b5c46c", "#c58ae0", "#6ccbd9", "#e07a6c"];

function hue(label: string): string {
  let hash = 0;
  for (const ch of label) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  return PALETTE[hash % PALETTE.length] ?? "#999";
}

function level(text: string): string {
  const t = text.toLowerCase();
  if (/\b(error|fatal|panic|exception|crit)\b/.test(t)) return "err";
  if (/\b(warn|warning)\b/.test(t)) return "warn";
  return "";
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
    const row = document.createElement("div");
    row.className = `log ${level(text)}`;
    row.innerHTML =
      `<span class="time">${escapeHtml(stamp)}</span>` +
      (label ? `<span class="source" style="--src:${hue(label)}">${escapeHtml(label)}</span>` : "") +
      `<span class="message">${escapeHtml(text)}</span>`;
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
