import { apiError } from "./useStream.js";

type Json = Record<string, unknown>;

async function call(url: string, method: string, body?: Json): Promise<unknown> {
  const response = await fetch(url, {
    method,
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
    credentials: "same-origin",
  });
  if (!response.ok) throw new Error(await apiError(response));
  const text = await response.text();
  return text ? JSON.parse(text) : null;
}

function done(el: HTMLElement) {
  if (el.dataset.success) window.toast(el.dataset.success, "success");
  if (el.dataset.redirect) location.href = el.dataset.redirect;
  else if (el.hasAttribute("data-reload")) void window.revalidate();
}

document.addEventListener("submit", async (event) => {
  const form = event.target as HTMLFormElement;
  if (!form.dataset.api) return;
  event.preventDefault();
  const body: Json = {};
  for (const [key, value] of new FormData(form).entries()) body[key] = value as string;
  const button = form.querySelector<HTMLButtonElement>("button[type=submit]");
  if (button) button.disabled = true;
  try {
    await call(form.dataset.api, form.dataset.method || "POST", body);
    done(form);
  } catch (error) {
    window.toast((error as Error).message);
  } finally {
    if (button) button.disabled = false;
  }
});

function confirmLabel(el: HTMLElement) {
  return el.getAttribute("aria-label") || el.textContent?.trim() || "Confirm";
}

document.addEventListener("click", async (event) => {
  const el = (event.target as HTMLElement).closest<HTMLElement>("[data-api]");
  if (!el || el.tagName === "FORM") return;
  if (el.dataset.confirm && !(await window.ask({ title: "Are you sure?", message: el.dataset.confirm, confirm: confirmLabel(el) }))) return;
  const button = el as HTMLButtonElement;
  button.disabled = true;
  try {
    await call(el.dataset.api!, el.dataset.method || "POST", el.dataset.body ? (JSON.parse(el.dataset.body) as Json) : {});
    done(el);
  } catch (error) {
    window.toast((error as Error).message);
  } finally {
    button.disabled = false;
  }
});

export { call };
