import { apiError } from "./useStream.js";

document.addEventListener("change", async (event) => {
  const select = (event.target as HTMLElement).closest<HTMLSelectElement>("select[data-policy]");
  if (!select) return;
  try {
    const response = await fetch(select.dataset.policy!, {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ scope: select.dataset.scope, policy: select.value }),
    });
    if (!response.ok) throw new Error(await apiError(response));
    window.toast("Update policy saved", "success");
  } catch (error) {
    window.toast((error as Error).message, "error");
  }
  void window.revalidate();
});

export {};
