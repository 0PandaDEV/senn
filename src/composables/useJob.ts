import { icon } from "./useIcons.js";
import { apiError, stripAnsi } from "./useStream.js";

declare global {
  interface Window {
    runJob(url: string, title: string): Promise<void>;
  }
}

type State = "running" | "done" | "failed";
type Job = { id: number; title: string; key?: string; state: State; error?: string | null; lines: string[] };

const MAX_TABS = 8;

const pending = new Set<string>();
const awaiting = new Map<string, { redirect?: string }>();
const redirects = new Map<number, string | undefined>();
const own = new Set<number>();

const root = () => document.getElementById("jobs");
const strip = () => document.getElementById("jobs-tabs");
const panes = () => document.getElementById("jobs-panes");
const tab = (id: number) => strip()?.querySelector<HTMLElement>(`.jobs-tab[data-job-id="${id}"]`) ?? null;
const pane = (id: number) => panes()?.querySelector<HTMLElement>(`.jobs-body[data-job-id="${id}"]`) ?? null;
const tabs = () => [...(strip()?.querySelectorAll<HTMLElement>(".jobs-tab") ?? [])];

function remember(name: string, value: string | null) {
  document.cookie = value === null ? `${name}=; path=/; max-age=0; samesite=lax` : `${name}=${value}; path=/; max-age=31536000; samesite=lax`;
}

function apply() {
  const keys = new Set(pending);
  for (const t of tabs()) if (t.dataset.state === "running" && t.dataset.key) keys.add(t.dataset.key);
  const hits = keys.size ? [...document.querySelectorAll<HTMLElement>("[data-job]")].filter((e) => keys.has(encodeURI(e.dataset.job!))) : [];
  const want = new Set<Element>(hits);
  for (const el of hits) {
    const row = el.closest("tr.row");
    if (row) want.add(row);
  }
  for (const el of document.querySelectorAll("[data-busy]")) if (!want.has(el)) el.removeAttribute("data-busy");
  for (const el of want) if (!el.hasAttribute("data-busy")) el.setAttribute("data-busy", "");
}

function activeId(): number | null {
  const t = tabs().find((e) => e.hasAttribute("data-active"));
  return t ? Number(t.dataset.jobId) : null;
}

function select(id: number) {
  const panel = root();
  if (!panel) return;
  for (const t of tabs()) t.toggleAttribute("data-active", Number(t.dataset.jobId) === id);
  for (const p of panes()?.querySelectorAll<HTMLElement>(".jobs-body") ?? []) p.hidden = Number(p.dataset.jobId) !== id;
  remember("senn_job_tab", String(id));
  const body = pane(id);
  if (body) body.scrollTop = body.scrollHeight;
  tab(id)?.scrollIntoView({ block: "nearest", inline: "nearest" });
}

function expand() {
  const panel = root();
  if (panel?.hasAttribute("data-collapsed")) {
    panel.removeAttribute("data-collapsed");
    remember("senn_jobs_collapsed", null);
  }
}

function removeTab(id: number) {
  const wasActive = activeId() === id;
  const list = tabs();
  const index = list.findIndex((t) => Number(t.dataset.jobId) === id);
  tab(id)?.remove();
  pane(id)?.remove();
  own.delete(id);
  redirects.delete(id);
  const rest = tabs();
  const panel = root();
  if (!rest.length) {
    if (panel) panel.hidden = true;
  } else if (wasActive) {
    select(Number(rest[Math.min(index, rest.length - 1)]!.dataset.jobId));
  }
  apply();
}

function dismiss(id: number) {
  removeTab(id);
  void fetch(`/api/jobs/${id}/dismiss`, { method: "POST", credentials: "same-origin" });
}

function ensure(id: number, title: string, key: string, state: State) {
  const bar = strip();
  const body = panes();
  const panel = root();
  if (!bar || !body || !panel) return;
  let t = tab(id);
  if (!t) {
    t = document.createElement("div");
    t.className = "jobs-tab";
    t.dataset.jobId = String(id);
    t.innerHTML = `<button type="button" class="jobs-tab-main" role="tab" data-job-select="${id}"><i class="jobs-dot"></i><span class="jobs-tab-title"></span></button><button type="button" class="jobs-tab-x" data-job-dismiss="${id}" aria-label="Close">${icon("x", 12)}</button>`;
    t.querySelector(".jobs-tab-title")!.textContent = title;
    bar.append(t);
    const pre = document.createElement("pre");
    pre.className = "jobs-body";
    pre.dataset.jobId = String(id);
    pre.hidden = true;
    body.append(pre);
    const extra = tabs().filter((e) => e.dataset.state !== "running" && e !== t);
    while (tabs().length > MAX_TABS && extra.length) removeTab(Number(extra.shift()!.dataset.jobId));
  }
  t.dataset.key = key;
  t.dataset.state = state;
  panel.hidden = false;
}

const queued = new Map<number, string>();
let frame = 0;

function flush() {
  frame = 0;
  for (const [id, text] of queued) {
    const body = pane(id);
    if (!body) continue;
    const stick = body.scrollTop + body.clientHeight >= body.scrollHeight - 24;
    const last = body.lastChild;
    if (last instanceof Text) last.appendData(text);
    else body.appendChild(document.createTextNode(text));
    if (stick) body.scrollTop = body.scrollHeight;
  }
  queued.clear();
}

function append(id: number, text: string) {
  queued.set(id, (queued.get(id) ?? "") + stripAnsi(text) + "\n");
  if (!frame) frame = requestAnimationFrame(flush);
}

function setState(id: number, state: State) {
  const t = tab(id);
  if (t) t.dataset.state = state;
  apply();
}

async function resync() {
  flush();
  try {
    const response = await fetch("/api/jobs", { credentials: "same-origin" });
    if (!response.ok) return;
    const jobs = ((await response.json()) as Job[]).slice(-MAX_TABS);
    const keep = new Set(jobs.map((j) => j.id));
    for (const t of tabs()) if (!keep.has(Number(t.dataset.jobId))) removeTab(Number(t.dataset.jobId));
    for (const job of jobs) {
      ensure(job.id, job.title, job.key ?? "", job.state);
      const body = pane(job.id)!;
      let text = job.lines.map((l) => stripAnsi(l) + "\n").join("");
      if (job.error) text += `error: ${job.error}\n`;
      if (body.textContent!.length < text.length) {
        const stick = body.scrollTop + body.clientHeight >= body.scrollHeight - 24;
        body.textContent = text;
        if (stick) body.scrollTop = body.scrollHeight;
      }
    }
    if (activeId() === null && jobs.length) select(jobs[jobs.length - 1]!.id);
    apply();
  } catch {
    return;
  }
}

function onEvent(message: { t: string; [key: string]: unknown }) {
  if (message.t === "open" || message.t === "jobs") {
    void resync();
    return;
  }
  if (message.t !== "job") return;
  const id = message.id as number;
  if (message.ev === "start") {
    const key = (message.key as string) ?? "";
    ensure(id, message.title as string, key, "running");
    const mine = awaiting.get(key);
    if (mine) {
      own.add(id);
      redirects.set(id, mine.redirect);
      awaiting.delete(key);
    }
    pending.delete(key);
    expand();
    select(id);
    apply();
    return;
  }
  if (message.ev === "dismiss") {
    removeTab(id);
    return;
  }
  if (message.ev === "line") {
    append(id, message.line as string);
    return;
  }
  if (message.ev === "end") {
    const ok = message.ok as boolean;
    if (!ok && message.error) append(id, `error: ${message.error as string}`);
    flush();
    setState(id, ok ? "done" : "failed");
    const title = tab(id)?.querySelector(".jobs-tab-title")?.textContent ?? "";
    const lines = (pane(id)?.textContent ?? "").trim().split("\n");
    const summary = lines.reverse().find((l) => l.startsWith("» "))?.slice(2);
    const done = summary ? `${title}: ${summary}` : `${title} finished`;
    if (own.has(id) && ok) {
      const redirect = redirects.get(id);
      if (redirect) {
        window.toastNextPage(done);
        location.href = redirect;
        return;
      }
      window.toast(done, "success");
    }
    void window.revalidate();
  }
}

async function run(url: string, title: string, redirect?: string) {
  const key = encodeURI(url);
  awaiting.set(key, { redirect });
  pending.add(key);
  apply();
  try {
    const response = await fetch(url, {
      method: "POST",
      credentials: "same-origin",
      headers: { "x-job-title": encodeURIComponent(title), "x-job-key": key, "Content-Type": "application/json" },
      body: "{}",
    });
    if (!response.ok) throw new Error(await apiError(response));
  } catch (error) {
    awaiting.delete(key);
    pending.delete(key);
    apply();
    window.toast((error as Error).message, "error");
  }
}

window.onLive(onEvent);
window.runJob = run;

function confirmLabel(el: HTMLElement) {
  return el.getAttribute("aria-label") || el.textContent?.trim() || "Confirm";
}

document.addEventListener("click", async (event) => {
  const target = event.target as HTMLElement;
  const el = target.closest<HTMLElement>("[data-job]");
  if (el) {
    if (el.dataset.confirm && !(await window.ask({ title: "Are you sure?", message: el.dataset.confirm, confirm: confirmLabel(el) }))) return;
    void run(el.dataset.job!, el.dataset.title || "Output", el.dataset.redirect);
    return;
  }
  const pick = target.closest<HTMLElement>("[data-job-select]");
  if (pick) {
    select(Number(pick.dataset.jobSelect));
    return;
  }
  const close = target.closest<HTMLElement>("[data-job-dismiss]");
  if (close) {
    dismiss(Number(close.dataset.jobDismiss));
    return;
  }
  if (target.closest("#jobs-toggle")) {
    const panel = root();
    if (!panel) return;
    const collapsed = panel.toggleAttribute("data-collapsed");
    remember("senn_jobs_collapsed", collapsed ? "1" : null);
  }
});

function middle(event: MouseEvent): HTMLElement | null {
  if (event.button !== 1) return null;
  return (event.target as HTMLElement).closest<HTMLElement>("#jobs-tabs .jobs-tab");
}

document.addEventListener("mousedown", (event) => {
  if (middle(event)) event.preventDefault();
});

document.addEventListener("auxclick", (event) => {
  const el = middle(event);
  if (!el) return;
  event.preventDefault();
  dismiss(Number(el.dataset.jobId));
});

new MutationObserver((mutations) => {
  if (mutations.every((m) => (m.target as Element).closest?.("#jobs"))) return;
  apply();
}).observe(document.body, { childList: true, subtree: true });

const first = activeId();
if (first !== null) {
  const body = pane(first);
  if (body) body.scrollTop = body.scrollHeight;
}
apply();

export {};
