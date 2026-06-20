// Thin, typed wrappers around the Tauri IPC commands exposed by `src-tauri`.
//
// When the frontend is opened in a plain browser (e.g. `vite dev` without the
// desktop shell), `window.__TAURI_INTERNALS__` is absent. We detect that and
// fail gracefully so the UI can still render for design work.

import { invoke } from "@tauri-apps/api/core";
import type { Character, Notification, ServerStatus } from "./types";

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw new Error(`IPC '${cmd}' unavailable: not running inside the desktop shell`);
  }
  return invoke<T>(cmd, args);
}

export const api = {
  serverStatus: () => call<ServerStatus>("server_status"),
  listCharacters: () => call<Character[]>("list_characters"),
  // Runs the whole SSO flow on the backend (opens the browser, captures the
  // loopback redirect) and resolves with the newly-added character.
  login: () => call<Character>("login"),
  setActiveCharacter: (characterId: number) =>
    call<void>("set_active_character", { characterId }),
  removeCharacter: (characterId: number) =>
    call<void>("remove_character", { characterId }),
  listNotifications: () => call<Notification[]>("list_notifications"),
  unreadNotifications: () => call<number>("unread_notifications"),
  markNotificationsRead: () => call<void>("mark_notifications_read"),
  dismissNotification: (key: string) =>
    call<void>("dismiss_notification", { key }),
};
