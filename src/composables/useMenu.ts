function shell() {
  return document.querySelector<HTMLElement>(".shell");
}

function close() {
  shell()?.classList.remove("menu-open");
}

document.addEventListener("click", (event) => {
  const target = event.target as HTMLElement;
  if (target.closest("[data-menu]")) {
    shell()?.classList.toggle("menu-open");
    return;
  }
  if (target.closest("[data-menu-close], .sidebar a, .sidebar .signout")) close();
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") close();
});

export {};
