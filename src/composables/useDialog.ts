interface AskOptions {
  title: string;
  message?: string;
  confirm?: string;
}

interface AskTextOptions {
  title: string;
  value?: string;
  placeholder?: string;
  confirm?: string;
}

declare global {
  interface Window {
    ask(options: AskOptions): Promise<boolean>;
    askText(options: AskTextOptions): Promise<string | null>;
  }
}

function prompt(title: string, message: string | undefined, confirm: string, input?: HTMLInputElement): Promise<string | null> {
  const dialog = document.createElement("dialog");
  dialog.className = "dialog";
  const form = document.createElement("form");
  const heading = document.createElement("h2");
  heading.textContent = title;
  form.append(heading);
  if (message) {
    const text = document.createElement("p");
    text.className = "muted";
    text.textContent = message;
    form.append(text);
  }
  if (input) form.append(input);
  const actions = document.createElement("div");
  actions.className = "dialog-actions";
  const cancel = document.createElement("button");
  cancel.type = "button";
  cancel.className = "button";
  cancel.textContent = "Cancel";
  const ok = document.createElement("button");
  ok.type = "submit";
  ok.className = "button primary";
  ok.textContent = confirm;
  actions.append(cancel, ok);
  form.append(actions);
  dialog.append(form);
  document.body.append(dialog);
  dialog.showModal();
  (input ?? ok).focus();
  input?.select();
  return new Promise((resolve) => {
    let result: string | null = null;
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      result = input ? input.value : "";
      dialog.close();
    });
    cancel.addEventListener("click", () => dialog.close());
    dialog.addEventListener(
      "close",
      () => {
        dialog.remove();
        resolve(result);
      },
      { once: true },
    );
  });
}

window.ask = async ({ title, message, confirm = "Confirm" }) => (await prompt(title, message, confirm)) !== null;

window.askText = ({ title, value = "", placeholder = "", confirm = "Save" }) => {
  const input = document.createElement("input");
  input.value = value;
  input.placeholder = placeholder;
  input.required = true;
  input.autocomplete = "off";
  input.spellcheck = false;
  return prompt(title, undefined, confirm, input);
};

document.addEventListener("click", (event) => {
  const target = event.target as HTMLElement;
  const opener = target.closest<HTMLElement>("[data-open]");
  if (opener) {
    const dialog = document.querySelector<HTMLDialogElement>(opener.dataset.open ?? "");
    dialog?.showModal();
    dialog?.querySelector<HTMLElement>("input:not([type=hidden]), textarea")?.focus();
    return;
  }
  const closer = target.closest<HTMLElement>("[data-close]");
  if (closer) {
    closer.closest("dialog")?.close();
    if (closer.hasAttribute("data-reload")) location.reload();
    return;
  }
  if (target instanceof HTMLDialogElement && target.open) {
    const box = target.getBoundingClientRect();
    const inside =
      event.clientX >= box.left &&
      event.clientX <= box.right &&
      event.clientY >= box.top &&
      event.clientY <= box.bottom;
    if (!inside) target.close();
  }
});

document.addEventListener("click", (event) => {
  const button = (event.target as HTMLElement).closest<HTMLElement>("[data-copy]");
  if (!button) return;
  const source = document.querySelector<HTMLElement>(button.dataset.copy ?? "");
  if (!source) return;
  void navigator.clipboard.writeText(source.textContent ?? "").then(() => window.toast("Copied", "success"));
});



export {};
