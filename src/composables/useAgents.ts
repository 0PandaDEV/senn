import { apiError } from "./useStream.js";

async function post(url: string, body?: unknown): Promise<any> {
  const response = await fetch(url, {
    method: "POST",
    credentials: "same-origin",
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
  if (!response.ok) throw new Error(await apiError(response));
  const text = await response.text();
  return text ? JSON.parse(text) : null;
}

function showInstall(data: { token: string; url: string }) {
  const pre = document.getElementById("install-cmd");
  const dialog = document.getElementById("install-agent") as HTMLDialogElement | null;
  if (!pre || !dialog) return;
  pre.textContent = [
    "curl -fsSL https://raw.githubusercontent.com/0PandaDEV/senn/main/install.sh | sudo sh -s -- \\",
    `  --url ${data.url} \\`,
    `  --token ${data.token} \\`,
    "  --stacks-dir /opt/stacks",
  ].join("\n");
  document.querySelectorAll("dialog[open]").forEach((d) => (d as HTMLDialogElement).close());
  dialog.showModal();
}

function fields(form: HTMLFormElement): Record<string, FormDataEntryValue> {
  return Object.fromEntries(new FormData(form).entries());
}

document.addEventListener("submit", async (event) => {
  const form = event.target as HTMLFormElement;
  const button = form.querySelector<HTMLButtonElement>("button[type=submit]");
  try {
    if (form.hasAttribute("data-agent-create")) {
      event.preventDefault();
      if (button) button.disabled = true;
      const data = fields(form);
      showInstall(await post("/api/agents", { name: data.name }));
    } else if (form.hasAttribute("data-agent-edit")) {
      event.preventDefault();
      if (button) button.disabled = true;
      const data = fields(form);
      await post(`/api/agents/${data.id}/config`, {
        name: data.name,
        defaultPolicy: String(data.defaultPolicy),
        updateCron: String(data.updateCron ?? ""),
        selfUpdateCron: String(data.selfUpdateCron ?? ""),
        address: String(data.address ?? ""),
        channel: String(data.channel ?? "stable"),
        selfUpdate: data.selfUpdate !== "off",
      });
      (form.closest("dialog") as HTMLDialogElement).close();
      window.toast("Saved", "success");
      location.reload();
    } else if (form.dataset.stackCreate) {
      event.preventDefault();
      if (button) button.disabled = true;
      const data = fields(form);
      const created = await post(`/api/hosts/${form.dataset.stackCreate}/stacks`, { name: data.name, compose: data.compose });
      window.toastNextPage(`Saved ${created?.path ?? "stack"}`);
      location.href = `/hosts/${form.dataset.stackCreate}/stacks/${encodeURIComponent(String(data.name))}`;
    }
  } catch (error) {
    window.toast((error as Error).message);
  } finally {
    if (button) button.disabled = false;
  }
});

document.addEventListener("input", (event) => {
  const input = event.target as HTMLInputElement;
  if (input.name !== "name" || !input.form?.hasAttribute("data-stack-create")) return;
  const preview = input.form.querySelector<HTMLElement>("[data-path-preview]");
  if (!preview) return;
  const base = preview.dataset.pathPreview ?? "";
  preview.textContent = `${base}${input.value.trim() || "…"}/docker-compose.yml`;
});

document.addEventListener("click", async (event) => {
  const target = event.target as HTMLElement;
  const edit = target.closest<HTMLElement>("[data-edit-agent]");
  if (edit) {
    const form = document.querySelector<HTMLFormElement>("form[data-agent-edit]")!;
    (form.elements.namedItem("id") as HTMLInputElement).value = edit.dataset.editAgent!;
    (form.elements.namedItem("name") as HTMLInputElement).value = edit.dataset.name ?? "";
    (form.elements.namedItem("defaultPolicy") as HTMLSelectElement).value = edit.dataset.policy ?? "notify";
    (form.elements.namedItem("updateCron") as HTMLInputElement).value = edit.dataset.updateCron || "0 */6 * * *";
    (form.elements.namedItem("selfUpdateCron") as HTMLInputElement).value = edit.dataset.selfUpdateCron || "0 4 * * *";
    const channel = form.elements.namedItem("channel") as HTMLSelectElement;
    if (edit.dataset.channel && ![...channel.options].some((o) => o.value === edit.dataset.channel)) channel.add(new Option(`Pin to ${edit.dataset.channel}`, edit.dataset.channel));
    channel.value = edit.dataset.channel ?? "stable";
    (form.elements.namedItem("selfUpdate") as HTMLSelectElement).value = edit.dataset.selfUpdate ?? "on";
    const address = form.elements.namedItem("address") as HTMLInputElement;
    address.value = edit.dataset.address ?? "";
    address.placeholder = edit.dataset.detected ? `${edit.dataset.detected} (detected)` : "detected automatically";
    (document.getElementById("edit-agent") as HTMLDialogElement).showModal();
    return;
  }
  const token = target.closest<HTMLElement>("[data-token-agent]");
  if (token) {
    if (!(await window.ask({ title: "Create a new token?", message: "The current one stops working and the agent disconnects until install is run again with the new token.", confirm: "New token" }))) return;
    try {
      showInstall(await post(`/api/agents/${token.dataset.tokenAgent}/token`));
    } catch (error) {
      window.toast((error as Error).message);
    }
  }
});

export {};

function openFromHash() {
  if (location.hash !== "#add-agent") return;
  const dialog = document.getElementById("add-agent") as HTMLDialogElement | null;
  if (!dialog) return;
  history.replaceState(history.state, "", location.pathname + location.search);
  if (!dialog.open) dialog.showModal();
  dialog.querySelector<HTMLElement>("input:not([type=hidden])")?.focus();
}

new MutationObserver(openFromHash).observe(document.body, { childList: true, subtree: true });
window.addEventListener("hashchange", openFromHash);
openFromHash();
