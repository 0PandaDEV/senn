import { apiError, stripAnsi } from "./useStream.js";

declare global {
  interface Window {
    runJob(url: string, title: string): Promise<void>;
  }
}

type Job = { id: number; title: string; key?: string; state: "running" | "done" | "failed"; error?: string | null; started: number; lines: string[] };

const DISMISSED = "senn:job-dismissed";
const RECENT = 60 * 60;

let current = 0;
let busy: string | null = null;
let awaiting: { redirect?: string } | null = null;
const redirects = new Map<number, string | undefined>();
const own = new Set<number>();

let watched: HTMLElement | null = null;
const sizer = new ResizeObserver(() => {
  const root = watched;
  document.documentElement.style.setProperty("--jobs-h", `${root && !root.hidden ? root.offsetHeight : 0}px`);
});

function watch() {
  const root = document.getElementById("jobs");
  if (!root || root === watched) return;
  if (watched) sizer.unobserve(watched);
  watched = root;
  sizer.observe(root);
}

function panel() {
  watch();
  return {
    root: document.getElementById("jobs") as HTMLElement,
    title: document.getElementById("jobs-title") as HTMLElement,
    state: document.getElementById("jobs-state") as HTMLElement,
    body: document.getElementById("jobs-body") as HTMLElement,
  };
}

function append(text: string) {
  const body = panel().body;
  const stick = body.scrollTop + body.clientHeight >= body.scrollHeight - 24;
  body.appendChild(document.createTextNode(stripAnsi(text) + "\n"));
  if (stick) body.scrollTop = body.scrollHeight;
}

function apply() {
  const hits = busy ? [...document.querySelectorAll<HTMLElement>("[data-job]")].filter((e) => encodeURI(e.dataset.job!) === busy) : [];
  const want = new Set<Element>(hits);
  for (const el of hits) {
    const row = el.closest("tr.row");
    if (row) want.add(row);
  }
  for (const el of document.querySelectorAll("[data-busy]")) if (!want.has(el)) el.removeAttribute("data-busy");
  for (const el of want) if (!el.hasAttribute("data-busy")) el.setAttribute("data-busy", "");
}

function setBusy(key: string | null) {
  busy = key || null;
  apply();
}

function setState(state: Job["state"]) {
  const p = panel();
  p.state.textContent = state === "running" ? "running" : state === "done" ? "done" : "failed";
  p.state.dataset.tone = state === "running" ? "warn" : state === "done" ? "ok" : "bad";
}

function open(title: string, state: Job["state"]) {
  const p = panel();
  p.root.hidden = false;
  p.title.textContent = title;
  p.body.textContent = "";
  setState(state);
}

function dismissed(): number {
  try {
    return Number(localStorage.getItem(DISMISSED) ?? 0);
  } catch {
    return 0;
  }
}

async function restore() {
  try {
    const response = await fetch("/api/jobs", { credentials: "same-origin" });
    if (!response.ok) return;
    const jobs = (await response.json()) as Job[];
    const job = jobs[jobs.length - 1];
    if (!job || job.id <= dismissed()) return;
    if (job.state !== "running" && Date.now() / 1000 - job.started > RECENT) return;
    if (current === job.id && job.state !== "running") return;
    current = job.id;
    open(job.title, job.state);
    setBusy(job.state === "running" ? (job.key ?? null) : null);
    for (const line of job.lines) append(line);
    if (job.error) append(`error: ${job.error}`);
  } catch {
    return;
  }
}

function onEvent(message: { t: string; [key: string]: unknown }) {
  if (message.t === "open" || message.t === "jobs") {
    void restore();
    return;
  }
  if (message.t !== "job") return;
  const id = message.id as number;
  if (message.ev === "start") {
    current = id;
    if (awaiting) {
      own.add(id);
      redirects.set(id, awaiting.redirect);
      awaiting = null;
    }
    open(message.title as string, "running");
    setBusy((message.key as string) ?? null);
    return;
  }
  if (id !== current) return;
  if (message.ev === "line") {
    append(message.line as string);
    return;
  }
  if (message.ev === "end") {
    const ok = message.ok as boolean;
    if (!ok && message.error) append(`error: ${message.error as string}`);
    setState(ok ? "done" : "failed");
    setBusy(null);
    const title = panel().title.textContent ?? "";
    const lines = (panel().body.textContent ?? "").trim().split("\n");
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
  awaiting = { redirect };
  open(title, "running");
  setBusy(encodeURI(url));
  try {
    const response = await fetch(url, {
      method: "POST",
      credentials: "same-origin",
      headers: { "x-job-title": encodeURIComponent(title), "x-job-key": encodeURI(url), "Content-Type": "application/json" },
      body: "{}",
    });
    if (!response.ok) throw new Error(await apiError(response));
  } catch (error) {
    awaiting = null;
    setBusy(null);
    append(`error: ${(error as Error).message}`);
    setState("failed");
  }
}

window.onLive(onEvent);
void restore();

window.runJob = run;

function confirmLabel(el: HTMLElement) {
  return el.getAttribute("aria-label") || el.textContent?.trim() || "Confirm";
}

document.addEventListener("click", async (event) => {
  const el = (event.target as HTMLElement).closest<HTMLElement>("[data-job]");
  if (el) {
    if (el.dataset.confirm && !(await window.ask({ title: "Are you sure?", message: el.dataset.confirm, confirm: confirmLabel(el) }))) return;
    void run(el.dataset.job!, el.dataset.title || "Output", el.dataset.redirect);
    return;
  }
  if ((event.target as HTMLElement).closest("#jobs-close")) {
    try {
      localStorage.setItem(DISMISSED, String(current));
    } catch {
      return;
    }
    panel().root.hidden = true;
  }
});

watch();
new MutationObserver(watch).observe(document.body, { childList: true });
new MutationObserver(apply).observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ["data-busy"] });

export {};
