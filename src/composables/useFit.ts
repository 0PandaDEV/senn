const SELECTOR = ".logs, .files, .env-file, .log-picker";
const jobs = () => {
  const panel = document.getElementById("jobs");
  return panel && !panel.hidden ? panel.offsetHeight + 8 : 0;
};
const BOTTOM = () => (matchMedia("(max-width: 860px)").matches ? 16 : 24) + jobs();

let queued = false;

function apply() {
  for (const el of document.querySelectorAll<HTMLElement>(SELECTOR)) {
    if (!el.offsetParent) continue;
    const top = el.getBoundingClientRect().top + window.scrollY;
    const height = `${Math.max(0, window.innerHeight - top - BOTTOM())}px`;
    if (el.style.getPropertyValue("--fit-h") !== height) el.style.setProperty("--fit-h", height);
  }
}

function fit() {
  if (queued) return;
  queued = true;
  requestAnimationFrame(() => {
    queued = false;
    apply();
  });
}

new MutationObserver((mutations) => {
  if (mutations.some((m) => !(m.target as Element).closest?.(".logs-body, .jobs-body"))) fit();
}).observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ["hidden", "data-collapsed"] });
const main = document.querySelector(".main");
if (main) new ResizeObserver(fit).observe(main);
window.addEventListener("resize", fit);
document.addEventListener("senn:tab", fit);
apply();
fit();

export {};
