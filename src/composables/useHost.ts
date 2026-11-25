const SECTIONS = ["stacks", "containers", "images", "volumes", "networks", "logs", "files"];

function remember(id: string) {
  document.cookie = `senn_host=${encodeURIComponent(id)}; path=/; max-age=31536000; samesite=lax`;
}

function current(): { id: string; section: string } | null {
  const m = location.pathname.match(/^\/hosts\/([^/]+)(?:\/([^/]+))?/);
  if (!m) return null;
  return { id: decodeURIComponent(m[1]!), section: SECTIONS.includes(m[2] ?? "") ? m[2]! : "" };
}

function sync() {
  const here = current();
  if (here) remember(here.id);
}

document.addEventListener("change", (event) => {
  const select = (event.target as HTMLElement).closest<HTMLSelectElement>("[data-host-select]");
  if (select) {
    const here = current();
    remember(select.value);
    location.assign(`/hosts/${encodeURIComponent(select.value)}${here?.section ? `/${here.section}` : ""}`);
    return;
  }
  const picker = (event.target as HTMLElement).closest<HTMLElement>("[data-log-picker]");
  if (picker) showLogs(picker);
});

function showLogs(picker: HTMLElement) {
  const agent = picker.dataset.logPicker!;
  const ids = [...picker.querySelectorAll<HTMLInputElement>("input[type=checkbox]:checked")].map((i) => i.value);
  const count = picker.querySelector<HTMLElement>("[data-log-count]");
  if (count) count.textContent = `${ids.length} selected`;
  const url = new URL(location.href);
  if (ids.length) url.searchParams.set("c", ids.join(","));
  else url.searchParams.delete("c");
  history.replaceState(history.state, "", url);
  const view = document.querySelector<HTMLElement>("[data-log-view]");
  if (!view) return;
  view.innerHTML = ids.length
    ? `<div data-logs="/api/hosts/${encodeURIComponent(agent)}/logs?containers=${ids.map(encodeURIComponent).join(",")}" data-multi=""></div>`
    : `<section class="empty small"><p>Pick containers on the left.</p></section>`;
}

document.addEventListener("click", (event) => {
  const toggle = (event.target as HTMLElement).closest<HTMLElement>("[data-log-picker-toggle]");
  if (toggle) toggle.closest(".log-picker")?.classList.toggle("open");
});

document.addEventListener("click", (event) => {
  const head = (event.target as HTMLElement).closest<HTMLElement>("[data-log-group]");
  if (!head) return;
  const boxes = [...head.parentElement!.querySelectorAll<HTMLInputElement>("input[type=checkbox]")];
  const on = !boxes.every((b) => b.checked);
  for (const b of boxes) b.checked = on;
  const picker = head.closest<HTMLElement>("[data-log-picker]");
  if (picker) showLogs(picker);
});

let last = "";
function watch() {
  if (location.pathname === last) return;
  last = location.pathname;
  sync();
}

new MutationObserver(watch).observe(document.body, { childList: true, subtree: true });
window.addEventListener("popstate", watch);
watch();

export {};
