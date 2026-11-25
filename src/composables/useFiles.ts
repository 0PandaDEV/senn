import { icon } from "./useIcons.js";
import { apiError, escapeHtml } from "./useStream.js";
import { Editor, langFor } from "./useEditor.js";
import { parseEnv } from "./useEnv.js";

type Entry = { name: string; kind: "dir" | "file" | "symlink"; size: number; modified: number };

const COMPOSE = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];

function join(a: string, b: string): string {
  return [a, b].filter(Boolean).join("/");
}

function size(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1048576) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1048576).toFixed(1)} MB`;
}

async function api<T>(agent: string, path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`/api/hosts/${agent}${path}`, { credentials: "same-origin", ...init });
  if (!response.ok) throw new Error(await apiError(response));
  const text = await response.text();
  return (text ? JSON.parse(text) : null) as T;
}

function post<T>(agent: string, path: string, body: unknown): Promise<T> {
  return api<T>(agent, path, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
}

function kind(name: string, dir: boolean): string {
  if (dir) return "dir";
  const lang = langFor(name);
  if (lang !== "text") return lang;
  const ext = name.includes(".") ? name.split(".").pop()!.toLowerCase() : "";
  if (["md", "txt"].includes(ext)) return "doc";
  if (["js", "ts", "py", "rb", "go", "rs", "php", "lua"].includes(ext)) return "code";
  if (["html", "css", "xml", "svg"].includes(ext)) return "markup";
  return "other";
}

function mount(el: HTMLElement) {
  el.dataset.bound = "1";
  const agent = el.dataset.agent!;
  const root = el.dataset.root ?? "";
  const abs = (el.dataset.abs ?? "").replace(/\/$/, "");
  const disk = (path: string) => (abs ? `${abs}/${path}` : path);
  const expanded = new Set<string>();
  const children = new Map<string, Entry[]>();
  let selected = root;
  let open = "";
  let dirty = false;
  el.classList.add("files");
  el.innerHTML = `
    <div class="files-list">
      <div class="files-crumbs" data-mid></div>
      <div class="files-tools">
        <button type="button" class="button" data-act="new-file">New file</button>
        <button type="button" class="button" data-act="new-dir">New folder</button>
      </div>
      <div class="files-entries"></div>
    </div>
    <div class="files-editor">
      <div class="files-head">
        <span class="files-name muted" data-mid>No file open</span>
        <span class="files-dirty" title="Unsaved changes" aria-label="Unsaved changes" hidden></span>
        <button type="button" class="button primary" data-act="save" disabled>Save</button>
      </div>
      <div class="files-text"></div>
      <pre class="files-error" hidden></pre>
    </div>`;
  const crumbs = el.querySelector<HTMLElement>(".files-crumbs")!;
  const entries = el.querySelector<HTMLElement>(".files-entries")!;
  const text = new Editor();
  text.disabled = true;
  text.el.classList.add("files-text");
  el.querySelector(".files-text")!.replaceWith(text.el);
  const envs = new Map<string, Promise<Map<string, string> | null>>();
  const envFor = (path: string) => {
    const parts = path.split("/");
    if (parts.length < 2) return Promise.resolve(null);
    const stack = parts[0]!;
    if (!envs.has(stack)) {
      envs.set(
        stack,
        api<{ content: string }>(agent, `/file?path=${encodeURIComponent(`${stack}/.env`)}`)
          .then((d) => new Map(parseEnv(d.content).filter((l) => l.key).map((l) => [l.key!, l.value ?? ""])))
          .catch(() => new Map<string, string>()),
      );
    }
    return envs.get(stack)!;
  };
  const onEnv = (event: Event) => {
    if (!el.isConnected) {
      document.removeEventListener("senn:env", onEnv);
      return;
    }
    const detail = (event as CustomEvent<{ agent: string; stack: string; vars: [string, string][] }>).detail;
    if (detail.agent !== agent || detail.stack !== root) return;
    const vars = new Map(detail.vars);
    envs.set(root, Promise.resolve(vars));
    if (open && !open.endsWith("/.env")) text.setVars(vars);
  };
  document.addEventListener("senn:env", onEnv);
  const name = el.querySelector<HTMLElement>(".files-name")!;
  const flag = el.querySelector<HTMLElement>(".files-dirty")!;
  const save = el.querySelector<HTMLButtonElement>("[data-act=save]")!;
  const error = el.querySelector<HTMLElement>(".files-error")!;

  const setDirty = (value: boolean) => {
    dirty = value;
    flag.hidden = !value;
    save.disabled = !open || !value;
  };

  const confirmLeave = async () => !dirty || window.ask({ title: "Discard unsaved changes?", confirm: "Discard" });

  crumbs.textContent = abs ? disk(root) : root || "/";

  const rows = (dir: string, depth: number): string =>
    (children.get(dir) ?? [])
      .map((e) => {
        const path = join(dir, e.name);
        const isDir = e.kind === "dir";
        const isOpen = isDir && expanded.has(path);
        const cls = ["entry", isOpen ? "expanded" : "", !isDir && path === open ? "open" : "", isDir && path === selected ? "selected" : ""].filter(Boolean).join(" ");
        const actions = `<span class="actions"><button type="button" data-ren="${escapeHtml(path)}" title="Rename" aria-label="Rename">${icon("pencil", 14)}</button><button type="button" data-del="${escapeHtml(path)}" title="Delete" aria-label="Delete">${icon("trash", 14)}</button></span>`;
        const row = `<div class="${cls}" style="--depth:${depth}" data-${isDir ? "dir" : "file"}="${escapeHtml(path)}">
          <span class="glyph" data-kind="${kind(e.name, isDir)}">${icon(isDir ? (isOpen ? "folder-open" : "folder") : "file")}</span>
          <span class="name" data-mid>${escapeHtml(e.name)}</span>
          ${isDir ? "" : `<span class="size">${size(e.size)}</span>`}
          ${actions}
        </div>`;
        return isOpen ? row + rows(path, depth + 1) : row;
      })
      .join("");

  const draw = () => {
    entries.innerHTML = rows(root, 0) || `<div class="entry-empty muted">Empty folder</div>`;
  };

  async function load(dir: string) {
    children.set(dir, await api<Entry[]>(agent, `/fs?path=${encodeURIComponent(dir)}`));
  }

  async function list(autoOpen = false) {
    try {
      await Promise.all([root, ...expanded].map((dir) => load(dir).catch(() => children.delete(dir))));
      draw();
      if (autoOpen && !open) {
        const compose = (children.get(root) ?? []).find((e) => COMPOSE.includes(e.name));
        if (compose) await openFile(join(root, compose.name), true);
      }
    } catch (e) {
      entries.innerHTML = `<div class="entry err">${escapeHtml((e as Error).message)}</div>`;
    }
  }

  async function openFile(path: string, force = false) {
    if (!force && !(await confirmLeave())) return;
    try {
      const data = await api<{ content: string }>(agent, `/file?path=${encodeURIComponent(path)}`);
      open = path;
      text.setPath(path);
      text.setVars(null);
      text.value = data.content;
      text.disabled = false;
      void envFor(path).then((vars) => {
        if (open === path) text.setVars(path.endsWith("/.env") ? null : vars);
      });
      name.textContent = disk(path);
      name.classList.remove("muted");
      error.hidden = true;
      setDirty(false);
      for (const row of entries.querySelectorAll<HTMLElement>(".entry")) row.classList.toggle("open", row.dataset.file === path);
      const parent = path.split("/").slice(0, -1).join("/");
      if (parent.length >= root.length) selected = parent;
    } catch (e) {
      window.toast((e as Error).message);
    }
  }

  async function saveFile() {
    if (!open || !dirty) return;
    save.disabled = true;
    try {
      const saved = await post<{ path?: string }>(agent, "/file", { path: open, content: text.value, validate: true });
      error.hidden = true;
      setDirty(false);
      if (open.endsWith(".env")) envs.delete(open.split("/")[0]!);
      window.toast(`Saved ${saved?.path ?? disk(open)}`, "success");
      void window.revalidate();
    } catch (e) {
      error.textContent = (e as Error).message;
      error.hidden = false;
      save.disabled = false;
    }
  }

  el.addEventListener("click", async (event) => {
    const t = event.target as HTMLElement;
    const del = t.closest<HTMLElement>("[data-del]");
    if (del) {
      const path = del.dataset.del!;
      if (!(await window.ask({ title: `Delete ${path}?`, message: "This cannot be undone.", confirm: "Delete" }))) return;
      try {
        await post(agent, "/fs/remove", { path });
        for (const dir of [...expanded]) {
          if (dir === path || dir.startsWith(`${path}/`)) expanded.delete(dir);
        }
        children.delete(path);
        if (selected === path || selected.startsWith(`${path}/`)) selected = root;
        if (open === path || open.startsWith(`${path}/`)) {
          open = "";
          text.value = "";
          text.disabled = true;
          name.textContent = "No file open";
          setDirty(false);
        }
        await list();
        void window.revalidate();
      } catch (e) {
        window.toast((e as Error).message);
      }
      return;
    }
    const ren = t.closest<HTMLElement>("[data-ren]");
    if (ren) {
      const from = ren.dataset.ren!;
      const base = from.split("/").slice(0, -1).join("/");
      const next = await window.askText({ title: "Rename", value: from.split("/").pop(), confirm: "Rename" });
      if (!next || next.includes("/")) return;
      try {
        await post(agent, "/fs/rename", { from, to: join(base, next) });
        if (open === from) open = join(base, next);
        for (const dir of [...expanded]) {
          if (dir === from || dir.startsWith(`${from}/`)) expanded.delete(dir);
        }
        children.delete(from);
        if (selected === from || selected.startsWith(`${from}/`)) selected = root;
        await list();
        void window.revalidate();
      } catch (e) {
        window.toast((e as Error).message);
      }
      return;
    }
    const dir = t.closest<HTMLElement>("[data-dir]");
    if (dir && el.contains(dir)) {
      const path = dir.dataset.dir ?? root;
      selected = path;
      if (expanded.has(path)) {
        expanded.delete(path);
      } else {
        expanded.add(path);
        if (!children.has(path)) {
          try {
            await load(path);
          } catch (e) {
            expanded.delete(path);
            window.toast((e as Error).message);
          }
        }
      }
      draw();
      return;
    }
    const file = t.closest<HTMLElement>("[data-file]");
    if (file) {
      void openFile(file.dataset.file!);
      return;
    }
    const act = t.closest<HTMLElement>("[data-act]")?.dataset.act;
    if (act === "save") void saveFile();
    if (act === "new-file" || act === "new-dir") {
      const label = act === "new-file" ? "File name" : "Folder name";
      const entry = await window.askText({ title: label, confirm: "Create" });
      if (!entry || entry.includes("/") || entry.startsWith(".."))  return;
      const path = join(selected, entry);
      try {
        if (act === "new-dir") await post(agent, "/fs/mkdir", { path });
        else await post(agent, "/file", { path, content: "", validate: false });
        expanded.add(selected);
        await list();
        if (act === "new-file") await openFile(path, true);
        void window.revalidate();
      } catch (e) {
        window.toast((e as Error).message);
      }
    }
  });

  text.onInput(() => setDirty(true));
  text.textarea.addEventListener("keydown", (event) => {
    if ((event.ctrlKey || event.metaKey) && event.key === "s") {
      event.preventDefault();
      void saveFile();
    }
  });
  window.addEventListener("beforeunload", (event) => {
    if (dirty && el.isConnected) event.preventDefault();
  });

  void list(true);
}

function scan() {
  for (const el of document.querySelectorAll<HTMLElement>("[data-files]:not([data-bound])")) {
    if (!el.closest("[hidden]")) mount(el);
  }
}

new MutationObserver(scan).observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ["hidden"] });
document.addEventListener("senn:tab", scan);
scan();

export {};
