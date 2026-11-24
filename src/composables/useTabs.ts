const KEY = "senn:stack-tab";

function stored(): string {
  try {
    return sessionStorage.getItem(KEY) ?? "";
  } catch {
    return "";
  }
}

function remember(name: string) {
  try {
    sessionStorage.setItem(KEY, name);
  } catch {
    return;
  }
}

function select(bar: HTMLElement, name: string) {
  for (const b of bar.querySelectorAll<HTMLElement>("[data-tab]")) b.classList.toggle("active", b.dataset.tab === name);
  const scope = bar.parentElement;
  if (!scope) return;
  for (const pane of scope.querySelectorAll<HTMLElement>("[data-pane]")) pane.hidden = pane.dataset.pane !== name;
  document.dispatchEvent(new CustomEvent("senn:tab", { detail: name }));
}

document.addEventListener("click", (event) => {
  const tab = (event.target as HTMLElement).closest<HTMLElement>("[data-tab]");
  const bar = tab?.closest<HTMLElement>("[data-tabs]");
  if (!tab || !bar) return;
  select(bar, tab.dataset.tab!);
  remember(tab.dataset.tab!);
  history.replaceState(null, "", `#${tab.dataset.tab}`);
});

function restore() {
  const bar = document.querySelector<HTMLElement>("[data-tabs]");
  if (!bar) return;
  const wanted = location.hash.slice(1) || stored();
  if (!wanted || !bar.querySelector(`[data-tab="${wanted}"]`)) return;
  const current = bar.querySelector<HTMLElement>("[data-tab].active")?.dataset.tab;
  const visible = bar.parentElement?.querySelector<HTMLElement>(`[data-pane="${wanted}"]`)?.hidden === false;
  if (current === wanted && visible) return;
  select(bar, wanted);
  if (location.hash.slice(1) !== wanted) history.replaceState(null, "", `#${wanted}`);
}

new MutationObserver((mutations) => {
  if (mutations.every((m) => (m.target as Element).closest?.(".logs-body"))) return;
  restore();
}).observe(document.body, { childList: true, subtree: true });
restore();

export {};
