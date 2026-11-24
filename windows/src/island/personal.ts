// Tasks and Notepad → the page (src-tauri/src/personal.rs): read once at start, then every edit
// answers with the new state. The Tasks pill wears how many are still open.

import { Bridge } from "../core/bridge";
import { Personal, TASKS_ID, openTasks } from "../core/personal";
import { pillDefinition } from "../core/pills";
import { State } from "../core/state";

export function registerPersonalHandlers() {
  void Bridge.personalGet().then((data) => {
    if (!data) return;
    Personal.data = data;
    syncPillName();
    State.notify();
  });
  State.subscribe(syncPillName);
}

/** "Tarefas · 3" while three are open, else the pill's name. */
export function syncPillName() {
  const task = State.tasks.find((x) => x.id === TASKS_ID);
  if (!task) return;
  const base = pillDefinition(TASKS_ID)?.name ?? "Tarefas";
  const open = openTasks();
  const name = open > 0 ? `${base} · ${open}` : base;
  if (task.name !== name) task.name = name;
}
