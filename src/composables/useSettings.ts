import { icon } from "./useIcons.js";
import { apiError, escapeHtml } from "./useStream.js";

const SERVICES: Record<string, string> = {
  ntfy: "ntfy",
  ntfys: "ntfy",
  discord: "Discord",
  tgram: "Telegram",
  slack: "Slack",
  gotify: "Gotify",
  gotifys: "Gotify",
  pover: "Pushover",
  mmost: "Mattermost",
  mmosts: "Mattermost",
  pbul: "Pushbullet",
  json: "Webhook",
  jsons: "Webhook",
};

function service(url: string): string {
  const scheme = url.trim().split("://")[0]?.toLowerCase() ?? "";
  if (!url.includes("://")) return url.trim() ? "?" : "";
  return SERVICES[scheme] ?? "unknown";
}

function mount(form: HTMLFormElement) {
  form.dataset.bound = "1";
  const hidden = form.querySelector<HTMLInputElement>('input[name="targets"]')!;
  const list = form.querySelector<HTMLElement>("[data-targets]")!;

  const row = (value: string) => {
    const name = service(value);
    return `<div class="target">
      <span class="target-service${name === "unknown" || name === "?" ? " bad" : ""}">${escapeHtml(name || "new")}</span>
      <input class="target-url" value="${escapeHtml(value)}" placeholder="ntfys://my-topic" spellcheck="false" autocomplete="off">
      <button type="button" class="button icon-only ghost danger" data-target-remove title="Remove" aria-label="Remove">${icon("trash")}</button>
    </div>`;
  };

  const sync = () => {
    hidden.value = [...list.querySelectorAll<HTMLInputElement>(".target-url")].map((i) => i.value.trim()).filter(Boolean).join("\n");
    list.classList.toggle("empty", !list.children.length);
  };

  const render = (values: string[]) => {
    list.innerHTML = values.map(row).join("");
    sync();
  };

  render(hidden.value.split("\n").map((v) => v.trim()).filter(Boolean));

  list.addEventListener("input", (event) => {
    const input = (event.target as HTMLElement).closest<HTMLInputElement>(".target-url");
    if (!input) return;
    const badge = input.parentElement!.querySelector<HTMLElement>(".target-service")!;
    const name = service(input.value);
    badge.textContent = name || "new";
    badge.classList.toggle("bad", name === "unknown" || name === "?");
    sync();
  });

  form.addEventListener("click", (event) => {
    const t = event.target as HTMLElement;
    if (t.closest("[data-target-add]")) {
      list.insertAdjacentHTML("beforeend", row(""));
      sync();
      list.querySelector<HTMLInputElement>(".target:last-child .target-url")?.focus();
      return;
    }
    const test = t.closest<HTMLButtonElement>("[data-target-test]");
    if (test) {
      sync();
      test.disabled = true;
      fetch("/api/settings/notify/test", {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ targets: hidden.value }),
      })
        .then(async (response) => {
          if (!response.ok) throw new Error(await apiError(response));
          window.toast("Test notification sent", "success");
        })
        .catch((error: Error) => window.toast(error.message))
        .finally(() => (test.disabled = false));
      return;
    }
    const remove = t.closest<HTMLElement>("[data-target-remove]");
    if (remove) {
      remove.closest(".target")?.remove();
      sync();
    }
  });

  form.addEventListener("submit", sync, true);
}

document.addEventListener("change", (event) => {
  const input = (event.target as HTMLElement).closest<HTMLInputElement>("[data-avatar-file]");
  const file = input?.files?.[0];
  if (!input || !file) return;
  if (file.size > 512 * 1024) {
    window.toast("The image is too large, keep it under 512 KB");
    input.value = "";
    return;
  }
  const reader = new FileReader();
  reader.onload = () => void upload(String(reader.result));
  reader.readAsDataURL(file);
});

document.addEventListener("click", (event) => {
  if ((event.target as HTMLElement).closest("[data-avatar-remove]")) void upload("");
});

async function upload(data: string) {
  const response = await fetch("/api/account/avatar", {
    method: "POST",
    credentials: "same-origin",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ data }),
  });
  if (!response.ok) {
    window.toast(await apiError(response));
    return;
  }
  window.toast(data ? "Picture updated" : "Picture removed", "success");
  void window.revalidate();
}

function scan() {
  for (const form of document.querySelectorAll<HTMLFormElement>("form[data-settings]:not([data-bound])")) mount(form);
}

new MutationObserver(scan).observe(document.body, { childList: true, subtree: true });
scan();

export {};
