const MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const DAYS = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

function node<K extends keyof HTMLElementTagNameMap>(tag: K, className?: string, html?: string): HTMLElementTagNameMap[K] {
  const n = document.createElement(tag);
  if (className) n.className = className;
  if (html !== undefined) n.innerHTML = html;
  return n;
}

function same(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
}

const ARROW = (d: string) =>
  `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;
const PREV = ARROW('<path d="m15 18-6-6 6-6"/>');
const NEXT = ARROW('<path d="m9 18 6-6-6-6"/>');
const PREV_YEAR = ARROW('<path d="m11 17-5-5 5-5M18 17l-5-5 5-5"/>');
const NEXT_YEAR = ARROW('<path d="m6 17 5-5-5-5M13 17l5-5-5-5"/>');

export class DatePicker {
  readonly el: HTMLElement;
  onPick: (t: number) => void = () => {};
  private view = new Date();
  private selected = new Date();
  private title: HTMLElement;
  private grid: HTMLElement;
  private time: HTMLInputElement[] = [];

  constructor() {
    this.el = node("div", "dp");
    this.el.hidden = true;
    this.el.setAttribute("role", "dialog");
    const head = node("div", "dp-head");
    const button = (html: string, label: string, fn: () => void) => {
      const b = node("button", "dp-nav", html);
      b.type = "button";
      b.setAttribute("aria-label", label);
      b.addEventListener("click", fn);
      return b;
    };
    this.title = node("span", "dp-title");
    head.append(
      button(PREV_YEAR, "Previous year", () => this.move(0, -1)),
      button(PREV, "Previous month", () => this.move(-1, 0)),
      this.title,
      button(NEXT, "Next month", () => this.move(1, 0)),
      button(NEXT_YEAR, "Next year", () => this.move(0, 1)),
    );
    const weekdays = node("div", "dp-week");
    for (const d of DAYS) weekdays.append(node("span", undefined, d));
    this.grid = node("div", "dp-grid");
    const time = node("div", "dp-time");
    time.append(node("span", "muted", "Time"));
    for (const label of ["Hours", "Minutes", "Seconds"]) {
      const input = node("input");
      input.type = "text";
      input.inputMode = "numeric";
      input.maxLength = 2;
      input.setAttribute("aria-label", label);
      input.addEventListener("focus", () => input.select());
      input.addEventListener("input", () => {
        input.value = input.value.replace(/\D/g, "").slice(0, 2);
      });
      input.addEventListener("blur", () => this.clamp(input));
      this.time.push(input);
    }
    time.append(this.time[0]!, node("b", undefined, ":"), this.time[1]!, node("b", undefined, ":"), this.time[2]!);
    const actions = node("div", "dp-actions");
    const today = node("button", "dp-text", "Today");
    today.type = "button";
    today.addEventListener("click", () => {
      const now = new Date();
      this.selected = new Date(now.getFullYear(), now.getMonth(), now.getDate(), ...this.clock());
      this.view = new Date(now.getFullYear(), now.getMonth(), 1);
      this.render();
    });
    const set = node("button", "button primary dp-set", "Set date");
    set.type = "button";
    set.addEventListener("click", () => this.commit());
    actions.append(today, set);
    this.el.append(head, weekdays, this.grid, time, actions);
    this.el.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        this.close();
      } else if (event.key === "Enter" && (event.target as HTMLElement).tagName === "INPUT") {
        event.preventDefault();
        this.commit();
      }
    });
  }

  get isOpen(): boolean {
    return !this.el.hidden;
  }

  open(t: number | null) {
    const date = t === null ? new Date() : new Date(t * 1000);
    this.selected = date;
    this.view = new Date(date.getFullYear(), date.getMonth(), 1);
    this.time[0]!.value = String(date.getHours()).padStart(2, "0");
    this.time[1]!.value = String(date.getMinutes()).padStart(2, "0");
    this.time[2]!.value = String(date.getSeconds()).padStart(2, "0");
    this.render();
    this.el.hidden = false;
  }

  close() {
    this.el.hidden = true;
  }

  private clamp(input: HTMLInputElement) {
    const max = input === this.time[0] ? 23 : 59;
    const n = Math.min(max, Number(input.value) || 0);
    input.value = String(n).padStart(2, "0");
  }

  private clock(): [number, number, number] {
    for (const input of this.time) this.clamp(input);
    return [Number(this.time[0]!.value), Number(this.time[1]!.value), Number(this.time[2]!.value)];
  }

  private move(months: number, years: number) {
    this.view = new Date(this.view.getFullYear() + years, this.view.getMonth() + months, 1);
    this.render();
  }

  private commit() {
    const [h, m, s] = this.clock();
    const d = this.selected;
    this.onPick(Math.floor(new Date(d.getFullYear(), d.getMonth(), d.getDate(), h, m, s).getTime() / 1000));
    this.close();
  }

  private render() {
    this.title.textContent = `${MONTHS[this.view.getMonth()]} ${this.view.getFullYear()}`;
    this.grid.replaceChildren();
    const offset = (this.view.getDay() + 6) % 7;
    const today = new Date();
    for (let i = 0; i < 42; i++) {
      const date = new Date(this.view.getFullYear(), this.view.getMonth(), 1 - offset + i);
      const b = node("button", "dp-day", String(date.getDate()));
      b.type = "button";
      b.classList.toggle("out", date.getMonth() !== this.view.getMonth());
      b.classList.toggle("today", same(date, today));
      b.classList.toggle("selected", same(date, this.selected));
      b.addEventListener("click", () => {
        this.selected = date;
        this.view = new Date(date.getFullYear(), date.getMonth(), 1);
        this.render();
      });
      b.addEventListener("dblclick", () => this.commit());
      this.grid.append(b);
    }
  }
}

export {};
