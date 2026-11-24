import { escapeHtml } from "./useStream.js";
import { icon } from "./useIcons.js";

const GLYPH = { error: "alert", success: "check", info: "info" } as const;

declare global {
  interface Window {
    toast(message: string, type?: "error" | "success" | "info"): void;
    revalidate(): Promise<void>;
    toastNextPage(message: string, type?: "error" | "success" | "info"): void;
  }
}

function toast(message: string, type: "error" | "success" | "info" = "error") {
  const host = document.getElementById("toasts");
  if (!host) return;
  const node = document.createElement("div");
  node.className = `toast ${type}`;
  node.setAttribute("role", type === "error" ? "alert" : "status");
  node.innerHTML = `<span class="toast-icon">${icon(GLYPH[type])}</span><span class="toast-msg">${escapeHtml(message)}</span><button type="button" class="toast-close" aria-label="Dismiss">${icon("x", 14)}</button>`;
  host.appendChild(node);
  const remove = () => {
    node.classList.add("leaving");
    setTimeout(() => node.remove(), 200);
  };
  node.querySelector(".toast-close")!.addEventListener("click", remove);
  setTimeout(remove, type === "error" ? 7000 : 3500);
}

window.toast = toast;

window.toastNextPage = (message, type = "success") => {
  sessionStorage.setItem("senn-toast", JSON.stringify({ message, type }));
};

const pending = sessionStorage.getItem("senn-toast");
if (pending) {
  sessionStorage.removeItem("senn-toast");
  const { message, type } = JSON.parse(pending) as { message: string; type: "error" | "success" | "info" };
  toast(message, type);
}

export {};
