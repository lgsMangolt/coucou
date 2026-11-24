// The Tasks and Notepad pills on the page: your own list and your own page of notes, kept by
// src-tauri/src/personal.rs. Nothing leaves the computer.

export const TASKS_ID = "integration_tasks";
export const PAD_ID = "integration_pad";
export const TASKS_COLOR = "#F5B83D";
export const PAD_COLOR = "#38BDF8";

export interface TodoTask {
  id: number;
  text: string;
  done: boolean;
}

/** Personal in src-tauri/src/personal.rs. */
export interface PersonalData {
  /** Newest first. */
  tasks: TodoTask[];
  pad: string;
}

/** The page's copy of what Rust keeps. */
export const Personal = {
  data: { tasks: [], pad: "" } as PersonalData,
};

export function openTasks(d: PersonalData = Personal.data): number {
  return d.tasks.filter((t) => !t.done).length;
}
