const rendered = new WeakMap<HTMLElement, string>();
const full = new WeakMap<HTMLElement, string>();

function overflows(el: HTMLElement): boolean {
  const style = getComputedStyle(el);
  const avail = el.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
  const range = document.createRange();
  range.selectNodeContents(el);
  return range.getBoundingClientRect().width > avail + 0.5;
}

function fit(el: HTMLElement) {
  const current = el.textContent ?? "";
  if (rendered.get(el) !== current) full.set(el, current);
  const text = full.get(el) ?? current;
  const set = (value: string) => {
    if (el.textContent !== value) el.textContent = value;
    rendered.set(el, value);
  };
  set(text);
  if (!el.title || el.title === rendered.get(el) || el.dataset.midTitle) {
    el.title = text;
    el.dataset.midTitle = "1";
  }
  if (!overflows(el) || text.length < 6) return;
  const cut = (keep: number) => {
    const head = Math.ceil(keep * 0.4);
    return `${text.slice(0, head)}…${text.slice(text.length - (keep - head))}`;
  };
  let lo = 1;
  let hi = text.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    set(cut(mid));
    if (!overflows(el)) lo = mid;
    else hi = mid - 1;
  }
  set(cut(lo));
}

const queue = new Set<HTMLElement>();
let frame = 0;

function schedule(el: HTMLElement) {
  queue.add(el);
  if (frame) return;
  frame = requestAnimationFrame(() => {
    frame = 0;
    for (const item of queue) if (item.isConnected) fit(item);
    queue.clear();
  });
}

const widths = new WeakMap<HTMLElement, number>();
const sizes = new ResizeObserver((entries) => {
  for (const entry of entries) {
    const el = entry.target as HTMLElement;
    const width = Math.round(entry.contentRect.width);
    if (widths.get(el) === width) continue;
    widths.set(el, width);
    schedule(el);
  }
});
const tracked = new WeakSet<HTMLElement>();

function scan() {
  for (const el of document.querySelectorAll<HTMLElement>("[data-mid]")) {
    if (!tracked.has(el)) {
      tracked.add(el);
      sizes.observe(el);
    }
    if (rendered.get(el) !== el.textContent) schedule(el);
  }
}

new MutationObserver(scan).observe(document.body, { childList: true, subtree: true, characterData: true });
scan();

export {};
