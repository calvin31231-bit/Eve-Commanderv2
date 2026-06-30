// Lightweight localization (Phase 7). A string catalog keyed by a stable id,
// with English as the source/fallback and additional locales layered on top.
// Missing keys fall back to English, then to the key itself, so the UI is never
// blank in a partially-translated locale. The active language persists to
// localStorage (instant on launch) and components re-render via `useLang`.
//
// Coverage grows by adding entries to CATALOG; this ships the framework + the
// chrome (hub names, common actions) and a starter German locale.

import { useEffect, useState } from "react";

export type Lang = "en" | "de";

export const LANGUAGES: { code: Lang; label: string }[] = [
  { code: "en", label: "English" },
  { code: "de", label: "Deutsch" },
];

type Catalog = Record<string, string>;

const EN: Catalog = {
  "hub.home": "Home",
  "hub.character": "Character",
  "hub.economy": "Economy",
  "hub.combat": "Combat & Intel",
  "hub.navigation": "Navigation & Logistics",
  "hub.corp": "Corp & Fleet",
  "hub.tools": "Tools",
  "home.welcome": "Welcome, Capsuleer",
  "home.subtitle": "Your at-a-glance command center.",
  "home.customize": "Customize",
  "home.done": "Done",
  "common.language": "Language",
  "common.save": "Save",
  "common.loading": "Loading…",
};

// Starter German locale. Untranslated keys fall back to English automatically.
const DE: Catalog = {
  "hub.home": "Start",
  "hub.character": "Charakter",
  "hub.economy": "Wirtschaft",
  "hub.combat": "Kampf & Aufklärung",
  "hub.navigation": "Navigation & Logistik",
  "hub.corp": "Corp & Flotte",
  "hub.tools": "Werkzeuge",
  "home.welcome": "Willkommen, Kapselpilot",
  "home.subtitle": "Deine Kommandozentrale auf einen Blick.",
  "home.customize": "Anpassen",
  "home.done": "Fertig",
  "common.language": "Sprache",
  "common.save": "Speichern",
  "common.loading": "Laden…",
};

const CATALOG: Record<Lang, Catalog> = { en: EN, de: DE };

const STORAGE_KEY = "eve-commander-lang";
let current: Lang = readInitial();
const listeners = new Set<() => void>();

function readInitial(): Lang {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v === "en" || v === "de") return v;
  } catch {
    // localStorage may be unavailable (SSR/preview) — default to English.
  }
  return "en";
}

export function getLang(): Lang {
  return current;
}

export function setLang(lang: Lang): void {
  if (lang === current) return;
  current = lang;
  try {
    localStorage.setItem(STORAGE_KEY, lang);
  } catch {
    // Best-effort persistence; the in-memory value still applies this session.
  }
  listeners.forEach((fn) => fn());
}

/** Translate a key for the active language, falling back to English then the key. */
export function t(key: string): string {
  return CATALOG[current][key] ?? CATALOG.en[key] ?? key;
}

/** Subscribe a component to language changes; returns the active language. */
export function useLang(): Lang {
  const [, force] = useState(0);
  useEffect(() => {
    const fn = () => force((n) => n + 1);
    listeners.add(fn);
    return () => {
      listeners.delete(fn);
    };
  }, []);
  return current;
}
