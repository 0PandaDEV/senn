import { escapeHtml } from "./useStream.js";

type Item = { label: string; insert: string; detail?: string; reopen?: boolean };

const TOP = ["services", "volumes", "networks", "configs", "secrets", "name", "include"];
const SERVICE = [
  "image", "build", "container_name", "command", "entrypoint", "environment", "env_file", "ports", "expose", "volumes",
  "networks", "network_mode", "depends_on", "restart", "healthcheck", "labels", "deploy", "logging", "user", "working_dir",
  "hostname", "domainname", "cap_add", "cap_drop", "devices", "dns", "extra_hosts", "mem_limit", "mem_reservation", "cpus",
  "cpu_shares", "privileged", "read_only", "security_opt", "stop_grace_period", "stop_signal", "sysctls", "tmpfs", "ulimits",
  "profiles", "pull_policy", "init", "ipc", "pid", "shm_size", "platform", "tty", "stdin_open", "group_add", "secrets",
  "configs", "runtime", "userns_mode", "links", "external_links",
];
const NESTED: Record<string, string[]> = {
  healthcheck: ["test", "interval", "timeout", "retries", "start_period", "start_interval", "disable"],
  build: ["context", "dockerfile", "dockerfile_inline", "args", "target", "cache_from", "cache_to", "labels", "network", "ssh", "secrets", "platforms", "pull", "no_cache", "tags"],
  deploy: ["resources", "restart_policy", "replicas", "labels", "mode", "placement", "update_config", "rollback_config"],
  logging: ["driver", "options"],
  options: ["max-size", "max-file", "compress", "tag"],
  resources: ["limits", "reservations"],
  limits: ["cpus", "memory", "pids"],
  reservations: ["cpus", "memory", "devices"],
  restart_policy: ["condition", "delay", "max_attempts", "window"],
  ulimits: ["nofile", "nproc", "memlock", "core"],
  nofile: ["soft", "hard"],
  dependency: ["condition", "restart", "required"],
  volume: ["driver", "driver_opts", "external", "labels", "name"],
  network: ["driver", "driver_opts", "external", "internal", "attachable", "ipam", "labels", "name", "enable_ipv6"],
  ipam: ["driver", "config", "options"],
};
const VALUES: Record<string, string[]> = {
  restart: ["no", "always", "on-failure", "unless-stopped"],
  pull_policy: ["always", "never", "missing", "build", "daily", "weekly"],
  condition: ["service_started", "service_healthy", "service_completed_successfully", "on-failure", "any", "none"],
  network_mode: ["host", "bridge", "none"],
  driver: ["json-file", "local", "journald", "syslog", "none", "bridge", "overlay", "macvlan"],
  stop_signal: ["SIGTERM", "SIGINT", "SIGQUIT", "SIGKILL"],
  mode: ["replicated", "global"],
};
const BOOLEAN = new Set(["init", "privileged", "read_only", "tty", "stdin_open", "external", "internal", "attachable", "disable", "required", "restart", "pull", "no_cache", "enable_ipv6"]);
const LIST = new Set(["ports", "expose", "cap_add", "cap_drop", "devices", "dns", "extra_hosts", "profiles", "security_opt", "tmpfs", "env_file", "links", "external_links", "group_add", "cache_from", "cache_to", "platforms", "tags", "include"]);
const MAP = new Set(["services", "environment", "labels", "healthcheck", "build", "deploy", "logging", "ulimits", "sysctls", "resources", "limits", "reservations", "restart_policy", "depends_on", "args", "options", "driver_opts", "ipam", "placement", "update_config", "rollback_config"]);

type Line = { indent: number; key: string | null; dash: boolean };

function parse(line: string): Line {
  const m = /^(\s*)(-\s+)?([\w.-]+)?\s*(:)?/.exec(line)!;
  const indent = m[1]!.length + (m[2]?.length ?? 0);
  return { indent, key: m[4] ? (m[3] ?? null) : null, dash: !!m[2] };
}

function ancestors(lines: string[], row: number, indent: number): string[] {
  const path: string[] = [];
  let limit = indent;
  for (let i = row - 1; i >= 0 && limit > 0; i--) {
    const raw = lines[i]!;
    if (!raw.trim() || raw.trim().startsWith("#")) continue;
    const l = parse(raw);
    const lead = raw.length - raw.trimStart().length;
    if (lead < limit && l.key) {
      path.unshift(l.key);
      limit = lead;
    } else if (lead < limit) {
      limit = lead;
    }
  }
  return path;
}

function siblings(lines: string[], row: number, indent: number): Set<string> {
  const out = new Set<string>();
  for (const dir of [-1, 1]) {
    for (let i = row + dir; i >= 0 && i < lines.length; i += dir) {
      const raw = lines[i]!;
      if (!raw.trim()) continue;
      const lead = raw.length - raw.trimStart().length;
      if (lead < indent) break;
      if (lead === indent) {
        const l = parse(raw);
        if (l.key) out.add(l.key);
      }
    }
  }
  return out;
}

function topKeys(lines: string[], section: string): string[] {
  const out: string[] = [];
  let inside = false;
  for (const raw of lines) {
    if (/^\S/.test(raw)) inside = raw.startsWith(`${section}:`);
    else if (inside) {
      const m = /^ {2}([\w.-]+):/.exec(raw);
      if (m) out.push(m[1]!);
    }
  }
  return out;
}

function keyItem(key: string, indent: number): Item {
  const pad = " ".repeat(indent + 2);
  if (LIST.has(key)) return { label: key, insert: `${key}:\n${pad}- `, detail: "list" };
  if (MAP.has(key)) return { label: key, insert: `${key}:\n${pad}`, detail: "map" };
  return { label: key, insert: `${key}: `, reopen: key in VALUES || BOOLEAN.has(key) };
}

export function suggest(text: string, caret: number, vars: string[]): { from: number; items: Item[] } | null {
  const before = text.slice(0, caret);
  const row = before.split("\n").length - 1;
  const lineStart = before.lastIndexOf("\n") + 1;
  const prefix = before.slice(lineStart);
  const lines = text.split("\n");

  const v = /\$\{([A-Za-z0-9_]*)$/.exec(prefix);
  if (v) {
    return { from: caret - v[1]!.length, items: vars.map((name) => ({ label: name, insert: `${name}}`, detail: "variable" })) };
  }

  const value = /^(\s*)(?:-\s+)?([\w.-]+):\s+([\w.-]*)$/.exec(prefix);
  if (value) {
    const key = value[2]!;
    const list = VALUES[key] ?? (BOOLEAN.has(key) ? ["true", "false"] : []);
    return { from: caret - value[3]!.length, items: list.map((x) => ({ label: x, insert: x })) };
  }

  const item = /^(\s*)-\s+([\w.-]*)$/.exec(prefix);
  if (item) {
    const path = ancestors(lines, row, item[1]!.length + 1);
    const parent = path.at(-1);
    let names: string[] = [];
    if (parent === "depends_on" || parent === "links") names = topKeys(lines, "services").filter((n) => n !== path[1]);
    else if (parent === "networks" && path[0] === "services") names = topKeys(lines, "networks");
    else if (parent === "cap_add" || parent === "cap_drop") names = ["NET_ADMIN", "SYS_ADMIN", "SYS_PTRACE", "NET_RAW", "CHOWN", "ALL"];
    return names.length ? { from: caret - item[2]!.length, items: names.map((x) => ({ label: x, insert: x })) } : null;
  }

  const key = /^(\s*)([\w.-]*)$/.exec(prefix);
  if (!key) return null;
  const indent = key[1]!.length;
  const path = ancestors(lines, row, indent);
  let keys: string[] = [];
  if (path.length === 0) keys = TOP;
  else if (path[0] === "services" && path.length === 2) keys = SERVICE;
  else if (path[0] === "services" && path.length === 4 && path[2] === "depends_on") keys = NESTED.dependency!;
  else if (path[0] === "volumes" && path.length === 2) keys = NESTED.volume!;
  else if (path[0] === "networks" && path.length === 2) keys = NESTED.network!;
  else keys = NESTED[path.at(-1)!] ?? [];
  if (!keys.length) return null;
  const taken = siblings(lines, row, indent);
  return { from: caret - key[2]!.length, items: keys.filter((k) => !taken.has(k)).map((k) => keyItem(k, indent)) };
}

export class Completer {
  private menu: HTMLElement;
  private items: Item[] = [];
  private active = 0;
  private from = 0;
  private open = false;
  private charWidth = 0;

  constructor(
    private input: HTMLTextAreaElement,
    host: HTMLElement,
    private enabled: () => boolean,
    private vars: () => string[],
  ) {
    this.menu = document.createElement("div");
    this.menu.className = "complete";
    this.menu.hidden = true;
    host.append(this.menu);
    this.menu.addEventListener("mousedown", (event) => {
      event.preventDefault();
      const el = (event.target as HTMLElement).closest<HTMLElement>("[data-i]");
      if (el) this.accept(Number(el.dataset.i));
    });
    input.addEventListener("keydown", (event) => this.key(event));
    input.addEventListener("input", (event) => {
      const e = event as InputEvent;
      if (!this.enabled()) return;
      if (e.inputType === "insertText" && e.data && /[\w.{$: -]/.test(e.data)) this.update(false);
      else if (this.open && e.inputType.startsWith("delete")) this.update(false);
      else this.close();
    });
    input.addEventListener("blur", () => this.close());
    input.addEventListener("click", () => this.close());
    input.addEventListener("scroll", () => this.close());
  }

  private key(event: KeyboardEvent) {
    if (event.key === " " && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      if (this.enabled()) this.update(true);
      return;
    }
    if (!this.open) return;
    const handled = () => {
      event.preventDefault();
      event.stopImmediatePropagation();
    };
    if (event.key === "ArrowDown") {
      handled();
      this.highlight(this.active + 1);
    } else if (event.key === "ArrowUp") {
      handled();
      this.highlight(this.active - 1);
    } else if (event.key === "Enter" || event.key === "Tab") {
      handled();
      this.accept(this.active);
    } else if (event.key === "Escape") {
      handled();
      this.close();
    } else if (["ArrowLeft", "ArrowRight", "Home", "End", "PageUp", "PageDown"].includes(event.key)) {
      this.close();
    }
  }

  private update(force: boolean) {
    const caret = this.input.selectionStart;
    if (caret !== this.input.selectionEnd) return this.close();
    const result = suggest(this.input.value, caret, this.vars());
    if (!result) return this.close();
    const typed = this.input.value.slice(result.from, caret).toLowerCase();
    if (!typed && !force && !/[{:-]\s?$|\$\{$/.test(this.input.value.slice(0, caret))) return this.close();
    const starts = result.items.filter((i) => i.label.toLowerCase().startsWith(typed));
    const contains = result.items.filter((i) => !starts.includes(i) && i.label.toLowerCase().includes(typed));
    this.items = [...starts, ...contains].slice(0, 50);
    if (!this.items.length || (this.items.length === 1 && this.items[0]!.label.toLowerCase() === typed)) return this.close();
    this.from = result.from;
    this.active = 0;
    this.render();
  }

  private render() {
    this.menu.innerHTML = this.items
      .map((item, i) => `<div class="complete-item${i === this.active ? " active" : ""}" data-i="${i}"><span>${escapeHtml(item.label)}</span>${item.detail ? `<small>${escapeHtml(item.detail)}</small>` : ""}</div>`)
      .join("");
    this.menu.hidden = false;
    this.open = true;
    this.place();
  }

  private place() {
    const style = getComputedStyle(this.input);
    if (!this.charWidth) {
      const ctx = document.createElement("canvas").getContext("2d")!;
      ctx.font = `${style.fontSize} ${style.fontFamily}`;
      this.charWidth = ctx.measureText("0000000000").width / 10;
    }
    const before = this.input.value.slice(0, this.from);
    const row = before.split("\n").length - 1;
    const col = this.from - (before.lastIndexOf("\n") + 1);
    const lineHeight = parseFloat(style.lineHeight) || 20;
    const top = parseFloat(style.paddingTop) + (row + 1) * lineHeight - this.input.scrollTop + 2;
    const left = parseFloat(style.paddingLeft) + col * this.charWidth - this.input.scrollLeft - 8;
    const host = this.menu.parentElement!;
    const height = this.menu.offsetHeight;
    const above = top - lineHeight - height - 4;
    this.menu.style.top = `${top + height > host.clientHeight && above >= 0 ? above : top}px`;
    this.menu.style.left = `${Math.max(0, left)}px`;
  }

  private highlight(i: number) {
    if (!this.items.length) return;
    this.active = (i + this.items.length) % this.items.length;
    const nodes = this.menu.querySelectorAll<HTMLElement>(".complete-item");
    nodes.forEach((n, j) => n.classList.toggle("active", j === this.active));
    nodes[this.active]?.scrollIntoView({ block: "nearest" });
  }

  private accept(i: number) {
    const item = this.items[i];
    if (!item) return;
    const caret = this.input.selectionStart;
    this.close();
    this.input.focus();
    this.input.setSelectionRange(this.from, caret);
    if (!document.execCommand("insertText", false, item.insert)) {
      this.input.setRangeText(item.insert, this.from, caret, "end");
      this.input.dispatchEvent(new Event("input"));
    }
    if (item.reopen) this.update(true);
  }

  close() {
    if (!this.open) return;
    this.open = false;
    this.menu.hidden = true;
  }
}

export {};
