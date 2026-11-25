function shell() {
  return document.querySelector<HTMLElement>(".shell");
}

function close() {
  const el = shell();
  if (!el?.classList.contains("menu-open")) return;
  el.classList.remove("menu-open");
}

document.addEventListener("click", (event) => {
  const target = event.target as HTMLElement;
  if (target.closest("[data-menu]")) {
    if (shell()?.classList.contains("menu-open")) close();
    else shell()?.classList.add("menu-open");
    return;
  }
  if (target.closest("[data-menu-close], .sidebar a, .sidebar .signout")) close();
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") close();
});

export {};
