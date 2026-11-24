interface Spec {
  container: string;
  item: string;
  name: string;
  underline: boolean;
}

interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

const specs: Spec[] = [
  { container: ".sidebar", item: ".nav a.active", name: "indicator", underline: false },
  { container: ".tabs", item: "a.active", name: "indicator", underline: true },
  { container: ".tabbar", item: "button.active", name: "indicator", underline: false },
];

const last = new Map<string, Box>();
const watched = new WeakSet<HTMLElement>();
const resizer = new ResizeObserver(() => run());

function measure(container: HTMLElement, item: HTMLElement, underline: boolean): Box {
  const c = container.getBoundingClientRect();
  const r = item.getBoundingClientRect();
  const x = r.left - c.left - container.clientLeft + container.scrollLeft;
  const y = r.top - c.top - container.clientTop + container.scrollTop;
  return underline ? { x, y: y + r.height - 2, w: r.width, h: 2 } : { x, y, w: r.width, h: r.height };
}

function place(el: HTMLElement, box: Box) {
  el.style.width = `${box.w}px`;
  el.style.height = `${box.h}px`;
  el.style.transform = `translate(${box.x}px, ${box.y}px)`;
}

function sync(spec: Spec) {
  const container = document.querySelector<HTMLElement>(spec.container);
  if (!container) return;
  const item = container.querySelector<HTMLElement>(spec.item);
  let indicator = container.querySelector<HTMLElement>(`:scope > .${spec.name}`);
  if (!item) {
    indicator?.remove();
    return;
  }
  if (!watched.has(container)) {
    watched.add(container);
    resizer.observe(container);
  }
  const target = measure(container, item, spec.underline);
  if (!indicator) {
    indicator = document.createElement("span");
    indicator.className = spec.name;
    container.prepend(indicator);
    place(indicator, last.get(spec.container) ?? target);
    void indicator.offsetWidth;
    indicator.classList.add("animated");
  }
  place(indicator, target);
  last.set(spec.container, target);
}

function run() {
  for (const spec of specs) sync(spec);
  observer.takeRecords();
}

const observer = new MutationObserver(run);

function start() {
  run();
  observer.observe(document.body, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ["class"],
  });
}

window.addEventListener("resize", run);
void document.fonts.ready.then(run);
if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
else start();
