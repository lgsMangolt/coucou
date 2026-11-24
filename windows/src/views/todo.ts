// The Tasks card: your own list. Type, Enter adds it at the top; the circle ticks it; × deletes it;
// "Clear done" sweeps the ticked ones. Built once and synced in place, so the field keeps its text
// and the keyboard while the list changes.

import { h, clear, dot } from "./dom";
import { Bridge } from "../core/bridge";
import { Personal, TASKS_COLOR, TASKS_ID, openTasks, type PersonalData, type TodoTask } from "../core/personal";
import { pillDefinition } from "../core/pills";
import { State } from "../core/state";
import { language, t, tl } from "../i18n/i18n";

export interface TodoCardHost {
  /** The island takes the keyboard for the field, and gives it back. */
  keyboard(on: boolean): void;
  /** A task is being typed: the island stays open. */
  holdOpen(on: boolean): void;
}

export interface TodoCard {
  el: HTMLElement;
  sync(): void;
}

/** Every edit answers with the list as it stands: the page shows that. */
async function apply(change: Promise<PersonalData | null>) {
  const data = await change;
  if (!data) return;
  Personal.data = data;
  State.notify();
}

export function buildTodoCard(host: TodoCardHost): TodoCard {
  const count = h("span", { class: "wa-unread" });
  const head = h("div", { class: "int-head" }, dot(TASKS_COLOR, 7), h("b", { text: pillDefinition(TASKS_ID)?.name ?? "Tarefas" }), count);
  const input = h("input", { class: "td-field", type: "text", spellcheck: "true", autocomplete: "off", maxlength: "200" }) as HTMLInputElement;
  const add = h("button", { class: "td-add", text: "+", title: tl("Add") });
  const inputRow = h("div", { class: "td-inputrow" }, input, add);
  const list = h("div", { class: "td-list" });
  const clearDone = h("button", { class: "link-btn td-clear" });
  const el = h("div", { class: "int-card td-card" }, head, inputRow, list, clearDone);

  function submit() {
    const text = input.value.trim();
    if (!text) return;
    input.value = "";
    void apply(Bridge.personalAddTask(text));
    input.focus();
  }
  add.addEventListener("click", submit);
  input.addEventListener("focus", () => {
    host.keyboard(true);
    host.holdOpen(true);
  });
  input.addEventListener("blur", () => {
    host.keyboard(false);
    host.holdOpen(false);
  });
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      submit();
    } else if (e.key === "Escape") {
      // Out of the field, not the island folding away.
      e.preventDefault();
      e.stopPropagation();
      input.blur();
    }
  });
  clearDone.addEventListener("click", () => void apply(Bridge.personalClearDone()));

  function row(task: TodoTask): HTMLElement {
    const tick = h("button", { class: task.done ? "td-tick on" : "td-tick", text: task.done ? "✓" : "" });
    tick.addEventListener("click", () => void apply(Bridge.personalToggleTask(task.id)));
    const del = h("button", { class: "td-del", text: "×", title: tl("Delete") });
    del.addEventListener("click", () => void apply(Bridge.personalDeleteTask(task.id)));
    return h("div", { class: task.done ? "td-row done" : "td-row" }, tick, h("span", { class: "td-text", text: task.text }), del);
  }

  let key = "";

  function sync() {
    const tasks = Personal.data.tasks;
    const open = openTasks();
    const next = [language(), ...tasks.map((x) => `${x.id}|${x.done ? 1 : 0}|${x.text}`)].join("~");
    input.placeholder = t("Add a task…");
    if (next === key) return;
    key = next;
    count.textContent = open > 0 ? String(open) : "";
    count.style.display = open > 0 ? "" : "none";
    clear(list);
    if (tasks.length === 0) list.append(h("div", { class: "wa-empty", text: t("Nothing to do") }));
    else list.append(...tasks.map(row));
    const done = tasks.length - open;
    clearDone.style.display = done > 0 ? "" : "none";
    clearDone.textContent = t("Clear done");
  }

  return { el, sync };
}
