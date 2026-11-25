const SELECTOR = ".logs, .files, .env-file, .log-picker";
const MOBILE = matchMedia("(max-width: 860px)");
const BOTTOM = () => (MOBILE.matches ? 8 : 16);

const probe = document.createElement("div");
probe.style.cssText = "position:fixed;top:0;width:0;height:100lvh;visibility:hidden;pointer-events:none";
document.body.append(probe);

let queued = false;

function apply() {
  const view = window.innerHeight;
  const full = MOBILE.matches ? Math.max(view, probe.offsetHeight) : view;
  const pad = `${full - view}px`;
  if (document.documentElement.style.getPropertyValue("--fit-pad") !== pad) document.documentElement.style.setProperty("--fit-pad", pad);
  for (const el of document.querySelectorAll<HTMLElement>(SELECTOR)) {
    if (!el.offsetParent) continue;
    const top = el.getBoundingClientRect().top + window.scrollY;
    const height = `${Math.max(0, full - top - BOTTOM())}px`;
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
window.visualViewport?.addEventListener("resize", fit);
document.addEventListener("senn:tab", fit);
apply();
fit();

export {};
