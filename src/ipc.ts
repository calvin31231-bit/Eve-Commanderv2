// Thin, typed wrappers around the Tauri IPC commands exposed by `src-tauri`.
//
// When the frontend is opened in a plain browser (e.g. `vite dev` without the
// desktop shell), `window.__TAURI_INTERNALS__` is absent. We detect that and
// fail gracefully so the UI can still render for design work.

import { invoke } from "@tauri-apps/api/core";
import type { Character, ServerStatus } from "./types";

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
  beginLogin: () => call<string>("begin_login"),
  completeLogin: (code: string, oauthState: string) =>
    call<Character>("complete_login", { code, oauthState }),
  setActiveCharacter: (characterId: number) =>
    call<void>("set_active_character", { characterId }),
  removeCharacter: (characterId: number) =>
    call<void>("remove_character", { characterId }),
};
