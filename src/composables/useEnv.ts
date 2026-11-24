import { icon } from "./useIcons.js";
import { apiError, escapeHtml } from "./useStream.js";
import { Editor, refs } from "./useEditor.js";

type Line = {
  raw: string;
  key?: string;
  value?: string;
  quote?: "" | '"' | "'";
  prefix?: string;
};

type Row = { id: number; key: string; value: string; line: number; origKey: string; origValue: string; quote: "" | '"' | "'"; prefix: string };

const COMPOSE_NAME = /(^|\/)(docker-)?compose([.-][^/]*)?\.ya?ml$/i;

function isCompose(file: { path: string; content: string }): boolean {
  if (COMPOSE_NAME.test(file.path)) return true;
  return /\.ya?ml$/i.test(file.path) && /^services:\s*(#.*)?$/m.test(file.content);
}

const LINE = /^(\s*export\s+)?([A-Za-z_][A-Za-z0-9_.-]*)\s*=\s*(.*)$/;

function parseValue(rest: string): { value: string; quote: "" | '"' | "'" } {
  const first = rest[0];
  if (first === '"' || first === "'") {
    let i = 1;
    let out = "";
    while (i < rest.length) {
      const c = rest[i]!;
      if (first === '"' && c === "\\" && i + 1 < rest.length) {
        out += c + rest[i + 1];
        i += 2;
        continue;
      }
      if (c === first) return { value: out, quote: first };
      out += c;
      i++;
    }
    return { value: rest.slice(1), quote: first };
  }
  const hash = rest.search(/\s#/);
  return { value: (hash === -1 ? rest : rest.slice(0, hash)).trim(), quote: "" };
}

export function parseEnv(text: string): Line[] {
  if (text === "") return [];
  const body = text.endsWith("\n") ? text.slice(0, -1) : text;
  return body.split("\n").map((raw) => {
    const trimmed = raw.trim();
    if (!trimmed || trimmed.startsWith("#")) return { raw };
    const m = LINE.exec(raw);
    if (!m) return { raw };
    const { value, quote } = parseValue(m[3] ?? "");
    return { raw, key: m[2], value, quote, prefix: m[1] ?? "" };
  });
}

function format(key: string, value: string, quote: "" | '"' | "'", prefix: string): string {
  let q = quote;
  if (!q && (/[\s#"']/.test(value) || value.startsWith("\\"))) q = '"';
  if (q === '"') return `${prefix}${key}="${value.replace(/(?<!\\)"/g, '\\"')}"`;
  if (q === "'") return `${prefix}${key}='${value.replace(/'/g, "")}'`;
  return `${prefix}${key}=${value}`;
}

function mount(el: HTMLElement) {
  el.dataset.bound = "1";
  const agent = el.dataset.agent!;
  const stack = el.dataset.stack!;
  const dir = el.dataset.dir ?? stack;
  const composeName = el.dataset.compose ?? "";
  const envPath = `${stack}/.env`;
  let lines: Line[] = [];
  let rows: Row[] = [];
  let exists = true;
  let dirty = false;
  let nextId = 1;
  let tree: { path: string; content: string }[] = [];
  let current = "";
  let fileDirty = false;

  el.classList.add("env");
  el.innerHTML = `
    <div class="env-head">
      <div class="env-path"><code>${escapeHtml(dir)}/.env</code></div>
      <div class="env-actions">
        <button type="button" class="button" data-env-add>Add variable</button>
        <button type="button" class="button" data-env-save disabled>Save</button>
        <button type="button" class="button primary" data-env-apply disabled>Save and apply</button>
      </div>
    </div>
    <div class="env-missing" hidden></div>
    <div class="env-split">
      <div class="env-file">
        <div class="env-file-head">
          <select class="env-file-pick" data-select="env-file-select" aria-label="File"></select>
          <span class="files-dirty" hidden>modified</span>
          <button type="button" class="button" data-env-file-save disabled>Save file</button>
        </div>
        <div class="env-editor"></div>
      </div>
      <div class="env-vars">
        <div class="env-rows"></div>
      </div>
    </div>`;
  const body = el.querySelector<HTMLElement>(".env-rows")!;
  const missing = el.querySelector<HTMLElement>(".env-missing")!;
  const save = el.querySelector<HTMLButtonElement>("[data-env-save]")!;
  const apply = el.querySelector<HTMLButtonElement>("[data-env-apply]")!;
  const pick = el.querySelector<HTMLSelectElement>(".env-file-pick")!;
  const fileFlag = el.querySelector<HTMLElement>(".env-file .files-dirty")!;
  const fileSave = el.querySelector<HTMLButtonElement>("[data-env-file-save]")!;
  const editor = new Editor();
  el.querySelector(".env-editor")!.replaceWith(editor.el);

  const setDirty = (value: boolean) => {
    dirty = value;
    save.disabled = !value;
    apply.disabled = !value;
  };

  const setFileDirty = (value: boolean) => {
    fileDirty = value;
    fileFlag.hidden = !value;
    fileSave.disabled = !value;
  };

  const vars = () => new Map(rows.filter((r) => r.key).map((r) => [r.key, r.value]));

  function usage() {
    const used = new Map<string, Set<string>>();
    const needed = new Map<string, Set<string>>();
    for (const file of tree) {
      if (!isCompose(file)) continue;
      for (const ref of refs(file.content, file.path)) {
        if (!used.has(ref.name)) used.set(ref.name, new Set());
        used.get(ref.name)!.add(file.path);
        if (!ref.optional) {
          if (!needed.has(ref.name)) needed.set(ref.name, new Set());
          needed.get(ref.name)!.add(file.path);
        }
      }
    }
    return { used, needed };
  }

  function refresh() {
    const { used, needed } = usage();
    for (const r of rows) {
      const row = body.querySelector<HTMLElement>(`[data-row="${r.id}"]`);
      const key = row?.querySelector<HTMLElement>(".env-key");
      if (!row || !key) continue;
      const files = [...(used.get(r.key) ?? [])];
      const unused = !!r.key && files.length === 0;
      row.classList.toggle("unused", unused);
      key.title = !r.key ? "" : unused ? "Not referenced in any file of this stack" : `Used in ${files.join(", ")}`;
    }
    const have = new Set(rows.map((r) => r.key));
    const unset = [...needed.keys()].filter((n) => !have.has(n)).sort();
    missing.hidden = unset.length === 0;
    missing.innerHTML = unset.length
      ? `<span>Used but not set:</span>` +
        unset
          .map((n) => `<button type="button" class="chip bad" data-env-fill="${escapeHtml(n)}" title="Used in ${escapeHtml([...needed.get(n)!].join(", "))}">${escapeHtml(n)}</button>`)
          .join("")
      : "";
    editor.setVars(vars());
    document.dispatchEvent(new CustomEvent("senn:env", { detail: { agent, stack, vars: [...vars()] } }));
  }

  function render() {
    body.innerHTML = rows.length
      ? rows
          .map((r) => {
            return `<div class="env-row" data-row="${r.id}">
              <input class="env-key" value="${escapeHtml(r.key)}" placeholder="KEY" spellcheck="false" autocomplete="off">
              <input class="env-val" type="text" value="${escapeHtml(r.value)}" placeholder="value" spellcheck="false" autocomplete="off">
              <button type="button" class="button icon-only ghost danger" data-env-del title="Remove" aria-label="Remove">${icon("x")}</button>
            </div>`;
          })
          .join("")
      : `<div class="empty small"><p>${exists ? "No variables in this file." : "This stack has no .env file yet. Saving creates it."}</p></div>`;
    refresh();
  }

  function build(): string {
    const out: string[] = [];
    const byLine = new Map(rows.filter((r) => r.line >= 0).map((r) => [r.line, r]));
    lines.forEach((l, i) => {
      if (l.key === undefined) {
        out.push(l.raw);
        return;
      }
      const row = byLine.get(i);
      if (!row) return;
      if (row.key === row.origKey && row.value === row.origValue) out.push(l.raw);
      else out.push(format(row.key, row.value, row.quote, row.prefix));
    });
    for (const r of rows.filter((r) => r.line < 0 && r.key)) out.push(format(r.key, r.value, "", ""));
    return out.length ? out.join("\n") + "\n" : "";
  }

  function show(path: string) {
    const file = tree.find((f) => f.path === path);
    current = file ? path : "";
    editor.setPath(path);
    editor.value = file?.content ?? "";
    editor.disabled = !file;
    setFileDirty(false);
  }

  async function loadTree() {
    try {
      tree = (await fetchJson<{ path: string; content: string }[]>(`/api/hosts/${agent}/tree?path=${encodeURIComponent(stack)}`)).filter((f) => f.path !== ".env");
    } catch {
      tree = [];
    }
    pick.innerHTML = tree.map((f) => `<option value="${escapeHtml(f.path)}">${escapeHtml(f.path)}</option>`).join("");
    const first = tree.find((f) => f.path === composeName)?.path ?? tree[0]?.path ?? "";
    pick.value = first;
    show(first);
  }

  async function load() {
    try {
      const data = await fetchJson<{ content: string }>(`/api/hosts/${agent}/file?path=${encodeURIComponent(envPath)}`);
      lines = parseEnv(data.content);
      exists = true;
    } catch (e) {
      if (!/No such file|not found/i.test((e as Error).message)) {
        body.innerHTML = `<div class="banner bad">${escapeHtml((e as Error).message)}</div>`;
        return;
      }
      lines = [];
      exists = false;
    }
    rows = lines
      .map((l, i) => ({ l, i }))
      .filter(({ l }) => l.key !== undefined)
      .map(({ l, i }) => ({ id: nextId++, key: l.key!, value: l.value ?? "", line: i, origKey: l.key!, origValue: l.value ?? "", quote: l.quote ?? "", prefix: l.prefix ?? "" }));
    setDirty(false);
    render();
  }

  async function post(path: string, content: string, validate: boolean): Promise<string | undefined> {
    const res = await fetch(`/api/hosts/${agent}/file`, {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ path, content, validate }),
    });
    if (!res.ok) throw new Error(await apiError(res));
    return ((await res.json()) as { path?: string }).path;
  }

  async function persist(): Promise<boolean> {
    const seen = new Set<string>();
    for (const r of rows) {
      if (!r.key) continue;
      if (!/^[A-Za-z_][A-Za-z0-9_.-]*$/.test(r.key)) {
        window.toast(`"${r.key}" is not a valid variable name`);
        return false;
      }
      if (seen.has(r.key)) {
        window.toast(`${r.key} is defined twice`);
        return false;
      }
      seen.add(r.key);
    }
    rows = rows.filter((r) => r.key || r.line >= 0);
    try {
      const saved = await post(envPath, build(), false);
      window.toast(`Saved ${saved ?? `${dir}/.env`}`, "success");
      await load();
      return true;
    } catch (e) {
      window.toast((e as Error).message);
      return false;
    }
  }

  async function saveFile() {
    if (!current || !fileDirty) return;
    fileSave.disabled = true;
    try {
      const saved = await post(`${stack}/${current}`, editor.value, current === composeName);
      setFileDirty(false);
      window.toast(`Saved ${saved ?? current}`, "success");
      void window.revalidate();
    } catch (e) {
      window.toast((e as Error).message);
      fileSave.disabled = false;
    }
  }

  editor.onInput(() => {
    const file = tree.find((f) => f.path === current);
    if (file) file.content = editor.value;
    setFileDirty(true);
    refresh();
  });

  editor.textarea.addEventListener("keydown", (event) => {
    if ((event.ctrlKey || event.metaKey) && event.key === "s") {
      event.preventDefault();
      void saveFile();
    }
  });

  let shown = "";
  pick.addEventListener("focus", () => (shown = current));
  pick.addEventListener("change", async () => {
    const next = pick.value;
    if (fileDirty && !(await window.ask({ title: "Discard unsaved changes?", message: `${shown} has unsaved changes.`, confirm: "Discard" }))) {
      pick.value = shown;
      return;
    }
    if (fileDirty) {
      await loadTree();
      pick.value = next;
    }
    show(next);
  });

  el.addEventListener("input", (event) => {
    const input = event.target as HTMLInputElement;
    const rowEl = input.closest<HTMLElement>("[data-row]");
    const row = rows.find((r) => r.id === Number(rowEl?.dataset.row));
    if (!row) return;
    if (input.classList.contains("env-key")) row.key = input.value.trim();
    if (input.classList.contains("env-val")) row.value = input.value;
    setDirty(true);
    refresh();
  });

  el.addEventListener("click", async (event) => {
    const t = event.target as HTMLElement;
    if (t.closest("[data-env-add]")) {
      rows.push({ id: nextId++, key: "", value: "", line: -1, origKey: "", origValue: "", quote: "", prefix: "" });
      render();
      body.querySelector<HTMLInputElement>(".env-row:last-child .env-key")?.focus();
      setDirty(true);
      return;
    }
    const fill = t.closest<HTMLElement>("[data-env-fill]");
    if (fill) {
      rows.push({ id: nextId++, key: fill.dataset.envFill!, value: "", line: -1, origKey: "", origValue: "", quote: "", prefix: "" });
      render();
      body.querySelector<HTMLInputElement>(".env-row:last-child .env-val")?.focus();
      setDirty(true);
      return;
    }
    const del = t.closest<HTMLElement>("[data-env-del]");
    if (del) {
      const id = Number(del.closest<HTMLElement>("[data-row]")?.dataset.row);
      rows = rows.filter((r) => r.id !== id);
      render();
      setDirty(true);
      return;
    }
    if (t.closest("[data-env-file-save]")) void saveFile();
    if (t.closest("[data-env-save]")) void persist();
    if (t.closest("[data-env-apply]")) {
      if (await persist()) void window.runJob(`/api/hosts/${agent}/stacks/${encodeURIComponent(stack)}/up`, `${stack} · apply environment`);
    }
  });

  window.addEventListener("beforeunload", (event) => {
    if ((dirty || fileDirty) && el.isConnected) event.preventDefault();
  });

  void Promise.all([load(), loadTree()]).then(refresh);
}

async function fetchJson<T>(url: string): Promise<T> {
  const res = await fetch(url, { credentials: "same-origin" });
  if (!res.ok) throw new Error(await apiError(res));
  return (await res.json()) as T;
}

function scan() {
  for (const el of document.querySelectorAll<HTMLElement>("[data-env]:not([data-bound])")) {
    if (!el.closest("[hidden]")) mount(el);
  }
}

new MutationObserver(scan).observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ["hidden"] });
document.addEventListener("senn:tab", scan);
scan();
