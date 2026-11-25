const PRESETS: [string, string][] = [
  ["Hourly", "0 * * * *"],
  ["Every 6h", "0 */6 * * *"],
  ["Daily 04:00", "0 4 * * *"],
  ["Weekly Mon", "0 3 * * 1"],
  ["Monthly", "0 3 1 * *"],
];
const ALIASES: Record<string, string> = {
  "@hourly": "0 * * * *",
  "@daily": "0 0 * * *",
  "@midnight": "0 0 * * *",
  "@weekly": "0 0 * * 0",
  "@monthly": "0 0 1 * *",
  "@yearly": "0 0 1 1 *",
  "@annually": "0 0 1 1 *",
};
const MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const DAYS = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
const DAY_NAMES = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTH_NAMES = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const FIELDS = ["minute", "hour", "day", "month", "weekday"];

type Field = { set: Set<number>; any: boolean; text: string };
type Cron = { minute: Field; hour: Field; day: Field; month: Field; weekday: Field };

function field(text: string, min: number, max: number, names: string[]): Field {
  const set = new Set<number>();
  const value = (v: string) => {
    const i = names.indexOf(v.toLowerCase());
    if (i !== -1) return i + min;
    if (!/^\d+$/.test(v)) throw new Error(`"${v}" is not a number`);
    return Number(v);
  };
  for (const part of text.split(",")) {
    const [range, stepText] = part.split("/");
    const step = stepText === undefined ? 1 : Number(stepText);
    if (!Number.isInteger(step) || step < 1) throw new Error(`bad step in "${part}"`);
    let lo: number;
    let hi: number;
    if (range === "*" || range === "?") [lo, hi] = [min, max];
    else if (range!.includes("-")) {
      const [a, b] = range!.split("-");
      [lo, hi] = [value(a!), value(b!)];
    } else {
      lo = value(range!);
      hi = stepText === undefined ? lo : max;
    }
    if (lo < min || hi > max || lo > hi) throw new Error(`"${part}" must be within ${min}-${max}`);
    for (let v = lo; v <= hi; v += step) set.add(max === 7 && v === 7 ? 0 : v);
  }
  return { set, any: text === "*" || text === "?", text };
}

function parse(input: string): Cron {
  const text = ALIASES[input.trim()] ?? input.trim();
  const parts = text.split(/\s+/).filter(Boolean);
  if (parts.length !== 5) throw new Error(`needs 5 fields, got ${parts.length}`);
  return {
    minute: field(parts[0]!, 0, 59, []),
    hour: field(parts[1]!, 0, 23, []),
    day: field(parts[2]!, 1, 31, []),
    month: field(parts[3]!, 1, 12, MONTHS),
    weekday: field(parts[4]!, 0, 7, DAYS),
  };
}

function dayMatches(c: Cron, d: Date): boolean {
  const dom = c.day.set.has(d.getDate());
  const dow = c.weekday.set.has(d.getDay());
  if (c.day.any && c.weekday.any) return true;
  if (c.day.any) return dow;
  if (c.weekday.any) return dom;
  return dom || dow;
}

function next(c: Cron, from: Date, count: number): Date[] {
  const out: Date[] = [];
  const d = new Date(from);
  d.setSeconds(0, 0);
  d.setMinutes(d.getMinutes() + 1);
  const limit = from.getTime() + 400 * 86400000;
  while (out.length < count && d.getTime() < limit) {
    if (!c.month.set.has(d.getMonth() + 1) || !dayMatches(c, d)) {
      d.setDate(d.getDate() + 1);
      d.setHours(0, 0, 0, 0);
    } else if (!c.hour.set.has(d.getHours())) {
      d.setHours(d.getHours() + 1, 0, 0, 0);
    } else if (!c.minute.set.has(d.getMinutes())) {
      d.setMinutes(d.getMinutes() + 1);
    } else {
      out.push(new Date(d));
      d.setMinutes(d.getMinutes() + 1);
    }
  }
  return out;
}

const pad = (n: number) => String(n).padStart(2, "0");

function list(f: Field, names?: string[]): string {
  const values = [...f.set].sort((a, b) => a - b).map((v) => (names ? names[v - (names === MONTH_NAMES ? 1 : 0)]! : String(v)));
  return values.length > 1 ? `${values.slice(0, -1).join(", ")} and ${values.at(-1)}` : values[0] ?? "";
}

function step(f: Field): number | null {
  const m = /^\*\/(\d+)$/.exec(f.text);
  return m ? Number(m[1]) : null;
}

function describe(c: Cron): string {
  let when: string;
  const ms = step(c.minute);
  const hs = step(c.hour);
  if (c.minute.any && c.hour.any) when = "every minute";
  else if (ms && c.hour.any) when = `every ${ms} minutes`;
  else if (c.minute.set.size === 1 && c.hour.any) when = `every hour at minute ${list(c.minute)}`;
  else if (c.minute.set.size === 1 && hs) when = `every ${hs} hours at minute ${list(c.minute)}`;
  else if (c.minute.set.size === 1 && c.hour.set.size <= 4 && !c.hour.any)
    when = `at ${[...c.hour.set].sort((a, b) => a - b).map((h) => `${pad(h)}:${pad([...c.minute.set][0]!)}`).join(", ")}`;
  else when = `at minute ${c.minute.any ? "every" : list(c.minute)} of hour ${c.hour.any ? "every" : list(c.hour)}`;
  const days: string[] = [];
  if (!c.day.any) days.push(`on day ${list(c.day)} of the month`);
  if (!c.weekday.any) days.push(`on ${list(c.weekday, DAY_NAMES)}`);
  const day = days.length ? days.join(" or ") : c.hour.any || step(c.hour) ? "" : "every day";
  const month = c.month.any ? "" : `in ${list(c.month, MONTH_NAMES)}`;
  return [when, day, month].filter(Boolean).join(", ");
}

function format(d: Date): string {
  const today = new Date();
  const time = `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  if (d.toDateString() === today.toDateString()) return `today ${time}`;
  return `${d.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" })} ${time}`;
}

function expand(value: string): string[] {
  const text = ALIASES[value.trim()] ?? value.trim();
  const parts = text.split(/\s+/).filter(Boolean);
  return FIELDS.map((_, i) => parts[i] ?? "");
}

function enhance(input: HTMLInputElement) {
  if (input.dataset.cronBound) return;
  input.dataset.cronBound = "1";
  const initial = input.value;
  input.type = "hidden";
  input.required = false;
  const wrap = document.createElement("div");
  wrap.className = "cron-input";
  wrap.innerHTML = FIELDS.map(
    (name) => `<div class="cron-part"><input type="text" inputmode="text" autocomplete="off" spellcheck="false" aria-label="${name}" placeholder="*"><span>${name}</span></div>`,
  ).join("");
  const parts = [...wrap.querySelectorAll<HTMLInputElement>("input")];
  const help = document.createElement("div");
  help.className = "cron-help";
  help.innerHTML =
    `<div class="cron-presets">${PRESETS.map(([label, expr]) => `<button type="button" class="chip" data-expr="${expr}">${label}</button>`).join("")}</div>` +
    `<p class="cron-status"></p>`;
  const status = help.querySelector<HTMLElement>(".cron-status")!;
  input.after(wrap);
  const field = input.closest(".field");
  if (field) field.append(help);
  else wrap.after(help);

  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!;
  const raw = (v?: string) => (v === undefined ? setter.get!.call(input) : setter.set!.call(input, v));

  const update = () => {
    const value = parts.map((p) => p.value.trim() || "*").join(" ");
    raw(value);
    for (const b of help.querySelectorAll<HTMLElement>(".cron-presets button")) b.classList.toggle("active", b.dataset.expr === value);
    try {
      const c = parse(value);
      const runs = next(c, new Date(), 1);
      const text = describe(c);
      status.className = "cron-status ok";
      status.textContent = `${text.charAt(0).toUpperCase()}${text.slice(1)} · next ${runs.length ? format(runs[0]!) : "never"}`;
      parts[0]!.setCustomValidity("");
      wrap.classList.remove("bad");
    } catch (error) {
      status.className = "cron-status bad";
      status.textContent = (error as Error).message;
      parts[0]!.setCustomValidity((error as Error).message);
      wrap.classList.add("bad");
    }
  };

  const fill = (value: string) => {
    expand(value).forEach((v, i) => (parts[i]!.value = v));
    update();
  };

  parts.forEach((part, i) => {
    part.addEventListener("input", () => {
      if (/\s/.test(part.value)) {
        const pieces = part.value.split(/\s+/);
        if (pieces.filter(Boolean).length > 1 || part.value.trim().startsWith("@")) {
          const merged = [...parts.slice(0, i).map((p) => p.value), part.value].join(" ");
          fill(merged);
          parts[Math.min(4, merged.trim().split(/\s+/).length - 1)]!.focus();
          return;
        }
        part.value = part.value.replace(/\s+/g, "");
        if (parts[i + 1]) {
          parts[i + 1]!.focus();
          parts[i + 1]!.select();
        }
      }
      update();
    });
    part.addEventListener("keydown", (event) => {
      const start = part.selectionStart ?? 0;
      const end = part.selectionEnd ?? 0;
      const go = (j: number, at: "start" | "end") => {
        const target = parts[j];
        if (!target) return false;
        event.preventDefault();
        target.focus();
        const pos = at === "start" ? 0 : target.value.length;
        target.setSelectionRange(pos, pos);
        return true;
      };
      if (event.key === "ArrowLeft" && start === 0 && end === 0) go(i - 1, "end");
      else if (event.key === "ArrowRight" && start === part.value.length) go(i + 1, "start");
      else if (event.key === "Backspace" && start === 0 && end === 0) go(i - 1, "end");
      else if (event.key === " ") {
        event.preventDefault();
        if (parts[i + 1]) {
          parts[i + 1]!.focus();
          parts[i + 1]!.select();
        }
      }
    });
    part.addEventListener("paste", (event) => {
      const text = event.clipboardData?.getData("text") ?? "";
      if (!/\s/.test(text.trim()) && !text.trim().startsWith("@")) return;
      event.preventDefault();
      fill(text);
    });
    part.addEventListener("focus", () => part.select());
  });

  help.addEventListener("click", (event) => {
    const button = (event.target as HTMLElement).closest<HTMLButtonElement>("button[data-expr]");
    if (!button) return;
    fill(button.dataset.expr!);
  });

  Object.defineProperty(input, "value", {
    configurable: true,
    get() {
      return raw();
    },
    set(v: string) {
      fill(v);
    },
  });
  fill(initial || input.placeholder);
}

function scan() {
  for (const input of document.querySelectorAll<HTMLInputElement>("input[data-cron]:not([data-cron-bound])")) enhance(input);
}

new MutationObserver(scan).observe(document.body, { childList: true, subtree: true });
scan();

export {};
