import { icon } from "./useIcons.js";
import { escapeHtml } from "./useStream.js";

const VALUE = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")!;
const INDEX = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "selectedIndex")!;

let closeOpen: (() => void) | null = null;

function enhance(select: HTMLSelectElement) {
  if (select.dataset.enhanced) return;
  select.dataset.enhanced = "1";
  const wrap = document.createElement("div");
  wrap.className = `select ${select.dataset.select ?? ""}`.trim();
  select.parentNode!.insertBefore(wrap, select);
  wrap.appendChild(select);
  const button = document.createElement("button");
  button.type = "button";
  button.className = "select-button";
  button.setAttribute("aria-haspopup", "listbox");
  button.setAttribute("aria-expanded", "false");
  if (select.getAttribute("aria-label")) button.setAttribute("aria-label", select.getAttribute("aria-label")!);
  button.innerHTML = `<span class="select-label"></span>${icon("chevron", 14)}`;
  wrap.appendChild(button);
  const labelEl = button.querySelector<HTMLElement>(".select-label")!;
  if (select.hasAttribute("data-mid-label")) labelEl.dataset.mid = "";

  const label = () => {
    labelEl.textContent = select.selectedOptions[0]?.textContent ?? "";
    button.disabled = select.disabled || select.options.length === 0;
  };
  Object.defineProperty(select, "value", {
    configurable: true,
    get() {
      return VALUE.get!.call(this);
    },
    set(v: string) {
      VALUE.set!.call(this, v);
      label();
    },
  });
  Object.defineProperty(select, "selectedIndex", {
    configurable: true,
    get() {
      return INDEX.get!.call(this);
    },
    set(v: number) {
      INDEX.set!.call(this, v);
      label();
    },
  });
  new MutationObserver(label).observe(select, { childList: true, subtree: true, attributes: true });
  select.addEventListener("change", label);
  label();

  let menu: HTMLElement | null = null;
  let active = -1;

  const items = () => (menu ? [...menu.querySelectorAll<HTMLElement>(".select-option")] : []);

  const highlight = (i: number) => {
    const list = items();
    if (!list.length) return;
    active = (i + list.length) % list.length;
    list.forEach((el, n) => el.classList.toggle("active", n === active));
    list[active]!.scrollIntoView({ block: "nearest" });
  };

  const close = () => {
    window.removeEventListener("scroll", onScroll, true);
    window.removeEventListener("resize", close);
    menu?.remove();
    menu = null;
    wrap.classList.remove("open");
    button.setAttribute("aria-expanded", "false");
    if (closeOpen === close) closeOpen = null;
  };

  const choose = (value: string) => {
    close();
    button.focus();
    if (value === VALUE.get!.call(select)) return;
    VALUE.set!.call(select, value);
    label();
    select.dispatchEvent(new Event("change", { bubbles: true }));
  };

  const option = (o: HTMLOptionElement) =>
    `<div class="select-option" role="option" data-value="${escapeHtml(o.value)}" aria-selected="${o.selected}">${escapeHtml(o.textContent ?? "")}</div>`;

  const place = () => {
    if (!menu) return;
    const rect = button.getBoundingClientRect();
    const gap = 4;
    menu.style.minWidth = `${rect.width}px`;
    const height = menu.offsetHeight;
    const below = window.innerHeight - rect.bottom - gap;
    const top = below >= height || below >= rect.top - gap ? rect.bottom + gap : rect.top - gap - height;
    menu.style.top = `${Math.max(gap, top)}px`;
    if (wrap.classList.contains("right")) {
      menu.style.left = "auto";
      menu.style.right = `${Math.max(gap, window.innerWidth - rect.right)}px`;
    } else {
      const width = menu.offsetWidth;
      menu.style.left = `${Math.max(gap, Math.min(rect.left, window.innerWidth - width - gap))}px`;
    }
  };

  const onScroll = (event: Event) => {
    const target = event.target;
    if (target === document || (target instanceof Node && target.contains(wrap))) close();
  };

  const open = () => {
    closeOpen?.();
    let html = "";
    for (const child of select.children) {
      if (child instanceof HTMLOptGroupElement) {
        html += `<div class="select-group">${escapeHtml(child.label)}</div>`;
        for (const o of child.querySelectorAll("option")) html += option(o);
      } else if (child instanceof HTMLOptionElement) {
        html += option(child);
      }
    }
    menu = document.createElement("div");
    menu.className = "select-menu";
    menu.setAttribute("role", "listbox");
    menu.setAttribute("popover", "manual");
    menu.innerHTML = html;
    wrap.appendChild(menu);
    menu.showPopover();
    place();
    wrap.classList.add("open");
    button.setAttribute("aria-expanded", "true");
    closeOpen = close;
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", close);
    const list = items();
    highlight(Math.max(0, list.findIndex((el) => el.getAttribute("aria-selected") === "true")));
    menu.addEventListener("mousedown", (event) => event.preventDefault());
    menu.addEventListener("click", (event) => {
      const el = (event.target as HTMLElement).closest<HTMLElement>(".select-option");
      if (el) choose(el.dataset.value!);
    });
    menu.addEventListener("mousemove", (event) => {
      const el = (event.target as HTMLElement).closest<HTMLElement>(".select-option");
      if (el) highlight(items().indexOf(el));
    });
  };

  button.addEventListener("click", () => (menu ? close() : open()));
  button.addEventListener("blur", () => setTimeout(() => menu && !wrap.contains(document.activeElement) && close(), 0));
  button.addEventListener("keydown", (event) => {
    if (!menu) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(event.key)) {
        event.preventDefault();
        open();
      }
      return;
    }
    if (event.key === "ArrowDown") highlight(active + 1);
    else if (event.key === "ArrowUp") highlight(active - 1);
    else if (event.key === "Home") highlight(0);
    else if (event.key === "End") highlight(-1);
    else if (event.key === "Enter" || event.key === " ") {
      const el = items()[active];
      if (el) choose(el.dataset.value!);
    } else if (event.key === "Escape" || event.key === "Tab") {
      close();
      if (event.key === "Tab") return;
    } else return;
    event.preventDefault();
  });
}

document.addEventListener("mousedown", (event) => {
  if (closeOpen && !(event.target as HTMLElement).closest(".select.open")) closeOpen();
});

function scan() {
  for (const select of document.querySelectorAll<HTMLSelectElement>("select[data-select]:not([data-enhanced])")) enhance(select);
}

new MutationObserver(scan).observe(document.body, { childList: true, subtree: true });
scan();

export {};
