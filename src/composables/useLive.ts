let timer: number | undefined;
let running = false;
let again = false;

function busy(el: Element): boolean {
  if (el.closest("dialog[open]")) return true;
  const active = document.activeElement;
  return !!active && el.contains(active) && /^(INPUT|TEXTAREA|SELECT)$/.test(active.tagName);
}

function key(node: Node): string | null {
  if (!(node instanceof Element)) return null;
  return node.id || node.getAttribute("data-key") || node.getAttribute("data-row") || null;
}

function clientOnly(node: Node | null): boolean {
  return node instanceof Element && node.matches(".indicator, [data-client]");
}

function skip(node: ChildNode | null): ChildNode | null {
  while (node && clientOnly(node)) node = node.nextSibling;
  return node;
}

function owned(el: Element): boolean {
  return el.hasAttribute("data-bound");
}

function syncAttributes(from: Element, to: Element) {
  for (const attr of [...from.attributes]) {
    if (attr.name === "data-enhanced" || attr.name === "data-busy") continue;
    if (!to.hasAttribute(attr.name)) from.removeAttribute(attr.name);
  }
  for (const attr of to.attributes) {
    if (from.getAttribute(attr.name) !== attr.value) from.setAttribute(attr.name, attr.value);
  }
}

function same(a: Node, b: Node): boolean {
  if (a.nodeType !== b.nodeType) return false;
  if (!(a instanceof Element) || !(b instanceof Element)) return true;
  if (a.classList.contains("select") && b instanceof HTMLSelectElement) return true;
  return a.tagName === b.tagName && key(a) === key(b);
}

function morph(from: Node, to: Node) {
  if (from.nodeType === Node.TEXT_NODE || from.nodeType === Node.COMMENT_NODE) {
    if (from.nodeValue !== to.nodeValue) from.nodeValue = to.nodeValue;
    return;
  }
  if (!(from instanceof Element) || !(to instanceof Element)) return;
  if (from.classList.contains("select") && to instanceof HTMLSelectElement) {
    const inner = from.querySelector("select");
    if (inner) morph(inner, to);
    return;
  }
  if (owned(from)) {
    const changed = ["data-logs", "data-files", "data-env", "data-root", "data-stack"].some((a) => from.getAttribute(a) !== to.getAttribute(a));
    if (changed) from.replaceWith(document.importNode(to, true));
    return;
  }
  syncAttributes(from, to);
  if (from instanceof HTMLSelectElement && to instanceof HTMLSelectElement) {
    if (from.innerHTML !== to.innerHTML) {
      from.innerHTML = to.innerHTML;
      from.value = to.value;
    }
    return;
  }
  if ((from instanceof HTMLInputElement || from instanceof HTMLTextAreaElement) && document.activeElement === from) return;
  const wanted = [...to.childNodes];
  let cursor = skip(from.firstChild);
  for (const next of wanted) {
    cursor = skip(cursor);
    let match: ChildNode | null = null;
    const k = key(next);
    if (k) {
      for (let n = cursor; n; n = n.nextSibling) {
        if (!clientOnly(n) && key(n) === k && same(n, next)) {
          match = n;
          break;
        }
      }
    } else if (cursor && same(cursor, next)) {
      match = cursor;
    }
    if (match) {
      if (match !== cursor) from.insertBefore(match, cursor);
      morph(match, next);
      cursor = match.nextSibling;
    } else {
      from.insertBefore(document.importNode(next, true), cursor);
    }
  }
  while (cursor) {
    const rest = cursor.nextSibling;
    if (!clientOnly(cursor)) cursor.remove();
    cursor = rest;
  }
}

async function revalidate(): Promise<void> {
  if (running) {
    again = true;
    return;
  }
  running = true;
  try {
    const href = location.href;
    const response = await fetch(href, { credentials: "same-origin", headers: { "x-senn-live": "1" } });
    if (!response.ok || location.href !== href || response.redirected) return;
    const next = new DOMParser().parseFromString(await response.text(), "text/html");
    for (const current of document.querySelectorAll<HTMLElement>("[data-live]")) {
      const key = current.dataset.live;
      const fresh = next.querySelector<HTMLElement>(`[data-live="${key}"]`);
      if (!fresh || busy(current)) continue;
      morph(current, fresh);
    }
  } catch {
    return;
  } finally {
    running = false;
    if (again) {
      again = false;
      void revalidate();
    }
  }
}

window.revalidate = revalidate;

function schedule() {
  clearTimeout(timer);
  timer = window.setTimeout(() => void revalidate(), 350);
}

type LiveMessage = { t: string; [key: string]: unknown };

declare global {
  interface Window {
    onLive(handler: (message: LiveMessage) => void): void;
    liveSub(agent: string | null): void;
  }
}

const handlers: ((message: LiveMessage) => void)[] = [];
window.onLive = (handler) => void handlers.push(handler);

let current: WebSocket | null = null;
let subscribed: string | null = null;

function sendSub() {
  if (current?.readyState === WebSocket.OPEN) current.send(JSON.stringify({ t: "sub", agent: subscribed }));
}

window.liveSub = (agent) => {
  subscribed = agent;
  sendSub();
};

function connect(attempt = 0) {
  const url = `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/api/ws`;
  const socket = new WebSocket(url);
  current = socket;
  let opened = false;
  socket.addEventListener("open", () => {
    opened = true;
    if (subscribed) sendSub();
    if (attempt > 0) schedule();
    for (const handler of handlers) handler({ t: "open" });
  });
  socket.addEventListener("message", (event) => {
    let message: LiveMessage;
    try {
      message = JSON.parse(event.data) as LiveMessage;
    } catch {
      return;
    }
    if (message.t === "hub" && (message.kind !== "stats" || document.querySelector(".usage, .host-card"))) schedule();
    for (const handler of handlers) handler(message);
  });
  socket.addEventListener("close", () => {
    const delay = Math.min(15000, 500 * 2 ** (opened ? 0 : attempt));
    window.setTimeout(() => connect(opened ? 0 : attempt + 1), delay);
  });
}

if (location.pathname !== "/login" && location.pathname !== "/setup") {
  connect();
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") schedule();
  });
}

export {};
