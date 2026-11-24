const SELECTOR = [
  ".chart-stats",
  ".table-wrap",
  ".jobs-tabs",
  ".jobs-body",
  ".logs-body",
  ".files-entries",
  ".files-text",
  ".log-picker",
  ".sidebar",
  ".tabs",
  ".tabbar",
  ".tp-list",
  "pre.code",
].join(", ");

const tracked = new WeakMap<HTMLElement, boolean>();
const dirty = new Set<HTMLElement>();
let queued = false;

function measure(el: HTMLElement) {
  const x = el.scrollWidth - el.clientWidth > 1;
  const y = el.scrollHeight - el.clientHeight > 1;
  el.toggleAttribute("data-fade-on", x || y);
  el.toggleAttribute("data-fade-l", x && el.scrollLeft > 1);
  el.toggleAttribute("data-fade-r", x && el.scrollLeft + el.clientWidth < el.scrollWidth - 1);
  el.toggleAttribute("data-fade-t", y && el.scrollTop > 1);
  el.toggleAttribute("data-fade-b", y && el.scrollTop + el.clientHeight < el.scrollHeight - 1);
}

function flush() {
  queued = false;
  for (const el of dirty) if (el.isConnected) measure(el);
  dirty.clear();
}

function mark(el: HTMLElement) {
  dirty.add(el);
  if (queued) return;
  queued = true;
  requestAnimationFrame(flush);
}

const sizer = new ResizeObserver((entries) => {
  for (const entry of entries) mark(entry.target as HTMLElement);
});

function bare(el: HTMLElement): boolean {
  const cs = getComputedStyle(el);
  const clear = cs.backgroundColor === "rgba(0, 0, 0, 0)" || cs.backgroundColor === "transparent";
  const borderless = [cs.borderTopWidth, cs.borderRightWidth, cs.borderBottomWidth, cs.borderLeftWidth].every((w) => parseFloat(w) === 0);
  return clear && cs.backgroundImage === "none" && borderless;
}

function track(el: HTMLElement) {
  if (tracked.has(el)) return;
  const active = bare(el);
  tracked.set(el, active);
  if (!active) return;
  el.setAttribute("data-fade", "");
  sizer.observe(el);
  mark(el);
}

function scan(node: Node) {
  if (!(node instanceof HTMLElement)) return;
  if (node.matches(SELECTOR)) track(node);
  for (const el of node.querySelectorAll<HTMLElement>(SELECTOR)) track(el);
}

new MutationObserver((mutations) => {
  for (const m of mutations) {
    for (const node of m.addedNodes) scan(node);
    const owner = (m.target instanceof Element ? m.target : m.target.parentElement)?.closest<HTMLElement>(SELECTOR);
    if (owner && tracked.get(owner)) mark(owner);
  }
}).observe(document.body, { childList: true, subtree: true, characterData: true });

document.addEventListener(
  "scroll",
  (event) => {
    const el = event.target;
    if (el instanceof HTMLElement && tracked.get(el)) mark(el);
  },
  true,
);

scan(document.body);

export {};
