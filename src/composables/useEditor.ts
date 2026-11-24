import { escapeHtml } from "./useStream.js";

export type Lang = "yaml" | "env" | "shell" | "docker" | "json" | "conf" | "text";

export type Ref = { name: string; optional: boolean; line: number };

type Rule = { re: RegExp; cls: string; bol?: boolean; afterSpace?: boolean; vars?: boolean };

const VAR = /\$\{([A-Za-z_][A-Za-z0-9_]*)(?:(:?[-+?])[^}]*)?\}|(?<![$\\])\$([A-Za-z_][A-Za-z0-9_]*)/g;
const DOCKER_WORDS = "FROM|RUN|CMD|LABEL|EXPOSE|ENV|ADD|COPY|ENTRYPOINT|VOLUME|USER|WORKDIR|ARG|ONBUILD|STOPSIGNAL|HEALTHCHECK|SHELL|AS";

const STRING_RULES: Rule[] = [
  { re: /"(?:[^"\\]|\\.)*"?/y, cls: "t-string", vars: true },
  { re: /'[^']*'?/y, cls: "t-string" },
];

const RULES: Record<Lang, Rule[]> = {
  yaml: [
    { re: /#.*/y, cls: "t-comment", afterSpace: true },
    { re: /\s*(?:-\s+)*(?:"[^"]*"|'[^']*'|[^\s#:"'][^#:]*?)(?=:(?:\s|$))/y, cls: "t-key", bol: true },
    { re: /\s*-(?=\s|$)/y, cls: "t-punct", bol: true },
    ...STRING_RULES,
    { re: /[&*][\w-]+/y, cls: "t-keyword" },
    { re: /\b(?:true|false|yes|no|null|on|off)\b/y, cls: "t-bool" },
    { re: /-?\b\d+(?:\.\d+)?\b/y, cls: "t-number" },
    { re: /[:[\]{},|>]/y, cls: "t-punct" },
  ],
  env: [
    { re: /\s*#.*/y, cls: "t-comment", bol: true },
    { re: /\s*(?:export\s+)?[A-Za-z_][\w.-]*(?=\s*=)/y, cls: "t-key", bol: true },
    { re: /=/y, cls: "t-punct" },
    ...STRING_RULES,
    { re: /\s#.*/y, cls: "t-comment" },
  ],
  shell: [
    { re: /#.*/y, cls: "t-comment", afterSpace: true },
    ...STRING_RULES,
    { re: /\b(?:if|then|else|elif|fi|for|in|do|done|while|case|esac|function|return|export|local|set|exec)\b/y, cls: "t-keyword" },
    { re: /(?<=\s|^)--?[\w-]+/y, cls: "t-bool" },
    { re: /\b\d+\b/y, cls: "t-number" },
  ],
  docker: [
    { re: /\s*#.*/y, cls: "t-comment", bol: true },
    { re: new RegExp(`\\s*(?:${DOCKER_WORDS})\\b`, "iy"), cls: "t-keyword", bol: true },
    { re: new RegExp(`\\b(?:AS)\\b`, "y"), cls: "t-keyword" },
    ...STRING_RULES,
    { re: /(?<=\s)--?[\w-]+(?:=\S*)?/y, cls: "t-bool" },
    { re: /\b\d+\b/y, cls: "t-number" },
  ],
  json: [
    { re: /"(?:[^"\\]|\\.)*"(?=\s*:)/y, cls: "t-key" },
    ...STRING_RULES,
    { re: /\b(?:true|false|null)\b/y, cls: "t-bool" },
    { re: /-?\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b/y, cls: "t-number" },
    { re: /[{}[\],:]/y, cls: "t-punct" },
  ],
  conf: [
    { re: /\s*[#;].*/y, cls: "t-comment", bol: true },
    { re: /\s*\[[^\]]*\]/y, cls: "t-keyword", bol: true },
    { re: /\s*[\w.-]+(?=\s*[=:])/y, cls: "t-key", bol: true },
    ...STRING_RULES,
    { re: /\s#.*/y, cls: "t-comment" },
    { re: /\b(?:true|false|yes|no|on|off)\b/y, cls: "t-bool" },
    { re: /-?\b\d+(?:\.\d+)?\b/y, cls: "t-number" },
  ],
  text: [],
};

export function langFor(path: string): Lang {
  const name = path.split("/").pop()?.toLowerCase() ?? "";
  if (name === ".env" || name.startsWith(".env.") || name.endsWith(".env")) return "env";
  if (name === "dockerfile" || name.startsWith("dockerfile.") || name.endsWith(".dockerfile")) return "docker";
  const ext = name.includes(".") ? name.split(".").pop()! : "";
  if (ext === "yml" || ext === "yaml") return "yaml";
  if (ext === "json") return "json";
  if (["sh", "bash", "zsh"].includes(ext)) return "shell";
  if (["conf", "cfg", "ini", "toml", "properties"].includes(ext)) return "conf";
  return "text";
}

function bare(lang: Lang): boolean {
  return lang === "env" || lang === "shell" || lang === "docker";
}

export function refs(text: string, path = ""): Ref[] {
  const out: Ref[] = [];
  const lang = langFor(path);
  const loose = bare(lang);
  text.split("\n").forEach((raw, i) => {
    const line = lang === "yaml" ? raw.replace(/(^|\s)#.*$/, "$1") : raw;
    for (const m of line.matchAll(VAR)) {
      if (!m[1] && !loose) continue;
      const name = m[1] ?? m[3]!;
      out.push({ name, optional: m[2] === ":-" || m[2] === "-", line: i });
    }
  });
  return out;
}

function shorten(value: string): string {
  if (value === "") return '""';
  return value.length > 28 ? `${value.slice(0, 27)}…` : value;
}

export class Editor {
  readonly el: HTMLElement;
  private input: HTMLTextAreaElement;
  private highlight: HTMLElement;
  private gutter: HTMLElement;
  private lang: Lang = "text";
  private vars: Map<string, string> | null = null;
  private frame = 0;
  private listeners: (() => void)[] = [];

  constructor(options: { value?: string; path?: string; readonly?: boolean } = {}) {
    this.el = document.createElement("div");
    this.el.className = "editor";
    this.el.innerHTML = `<div class="editor-gutter"><div class="editor-lines"></div></div>
      <div class="editor-body"><pre class="editor-highlight" aria-hidden="true"></pre><textarea class="editor-input" spellcheck="false" wrap="off" autocapitalize="off" autocomplete="off"></textarea></div>`;
    this.input = this.el.querySelector("textarea")!;
    this.highlight = this.el.querySelector(".editor-highlight")!;
    this.gutter = this.el.querySelector(".editor-lines")!;
    if (options.path) this.lang = langFor(options.path);
    this.input.value = options.value ?? "";
    this.input.readOnly = !!options.readonly;
    this.input.addEventListener("input", () => {
      this.schedule();
      for (const fn of this.listeners) fn();
    });
    this.input.addEventListener("scroll", () => this.sync());
    this.input.addEventListener("keydown", (event) => {
      if (event.key === "Tab" && !event.shiftKey && !this.input.readOnly) {
        event.preventDefault();
        this.input.setRangeText("  ", this.input.selectionStart, this.input.selectionEnd, "end");
        this.input.dispatchEvent(new Event("input"));
      }
    });
    this.render();
  }

  get value(): string {
    return this.input.value;
  }

  set value(text: string) {
    this.input.value = text;
    this.input.scrollTop = 0;
    this.input.scrollLeft = 0;
    this.render();
  }

  get textarea(): HTMLTextAreaElement {
    return this.input;
  }

  set disabled(value: boolean) {
    this.input.disabled = value;
    this.el.classList.toggle("disabled", value);
  }

  setPath(path: string) {
    this.lang = langFor(path);
    this.render();
  }

  setVars(vars: Map<string, string> | null) {
    this.vars = vars;
    this.schedule();
  }

  onInput(fn: () => void) {
    this.listeners.push(fn);
  }

  focus() {
    this.input.focus();
  }

  private schedule() {
    cancelAnimationFrame(this.frame);
    this.frame = requestAnimationFrame(() => this.render());
  }

  private sync() {
    const x = this.input.scrollLeft;
    const y = this.input.scrollTop;
    this.highlight.style.transform = `translate(${-x}px, ${-y}px)`;
    this.gutter.style.transform = `translateY(${-y}px)`;
  }

  private varHtml(name: string, raw: string, optional: boolean): string {
    if (!raw.startsWith("${") && !bare(this.lang)) return escapeHtml(raw);
    let cls = "t-var";
    if (this.vars && !this.vars.has(name) && !optional) cls += " missing";
    return `<span class="${cls}">${escapeHtml(raw)}</span>`;
  }

  private withVars(text: string): string {
    let out = "";
    let last = 0;
    for (const m of text.matchAll(VAR)) {
      out += escapeHtml(text.slice(last, m.index));
      out += this.varHtml(m[1] ?? m[3]!, m[0], m[2] === ":-" || m[2] === "-");
      last = m.index! + m[0].length;
    }
    return out + escapeHtml(text.slice(last));
  }

  private line(text: string): string {
    const rules = RULES[this.lang];
    let out = "";
    let plain = "";
    let pos = 0;
    const flush = () => {
      out += this.withVars(plain);
      plain = "";
    };
    while (pos < text.length) {
      let hit = false;
      for (const rule of rules) {
        if (rule.bol && pos !== 0) continue;
        if (rule.afterSpace && pos !== 0 && !/\s/.test(text[pos - 1]!)) continue;
        rule.re.lastIndex = pos;
        const m = rule.re.exec(text);
        if (!m || m[0].length === 0) continue;
        flush();
        const body = rule.vars || rule.cls === "t-key" ? this.withVars(m[0]) : escapeHtml(m[0]);
        out += `<span class="${rule.cls}">${body}</span>`;
        pos += m[0].length;
        hit = true;
        break;
      }
      if (hit) continue;
      if (text[pos] === "$") {
        VAR.lastIndex = 0;
        const rest = text.slice(pos);
        const m = new RegExp(VAR.source, "y").exec(rest);
        if (m) {
          flush();
          out += this.varHtml(m[1] ?? m[3]!, m[0], m[2] === ":-" || m[2] === "-");
          pos += m[0].length;
          continue;
        }
      }
      plain += text[pos];
      pos++;
    }
    flush();
    return out;
  }

  private hint(text: string): { html: string; warn: boolean } {
    if (!this.vars) return { html: "", warn: false };
    const parts: string[] = [];
    const seen = new Set<string>();
    const names = new Set<string>();
    for (const m of text.matchAll(VAR)) {
      if (m[1] || bare(this.lang)) names.add(m[1] ?? m[3]!);
    }
    const label = (name: string, value: string) => (names.size > 1 ? `${name} = ${value}` : value);
    let warn = false;
    for (const m of text.matchAll(VAR)) {
      if (!m[1] && !bare(this.lang)) continue;
      const name = m[1] ?? m[3]!;
      if (seen.has(name)) continue;
      seen.add(name);
      const optional = m[2] === ":-" || m[2] === "-";
      if (this.vars.has(name)) {
        parts.push(`<span class="t-hint">${escapeHtml(label(name, shorten(this.vars.get(name)!)))}</span>`);
      } else if (!optional) {
        warn = true;
        parts.push(`<span class="t-hint warn">${escapeHtml(names.size > 1 ? `${name} not set` : "not set")}</span>`);
      }
    }
    return { html: parts.length ? `<span class="t-gap">  </span>${parts.join(" ")}` : "", warn };
  }

  private render() {
    const lines = this.input.value.split("\n");
    let code = "";
    let numbers = "";
    lines.forEach((text, i) => {
      const { html, warn } = this.hint(text);
      code += `${this.line(text)}${html}\n`;
      numbers += `<div class="${warn ? "warn" : ""}">${i + 1}</div>`;
    });
    this.highlight.innerHTML = code;
    this.gutter.innerHTML = numbers;
    this.sync();
  }
}
