import { useSyncExternalStore } from "react";
import { DARK_THEME, LIGHT_THEME, findTheme } from "./catalog";
import type { ThemeDefinition, ThemePreference } from "./model";

const STORAGE_KEY = "argusflow.theme";
const listeners = new Set<() => void>();
let preference: ThemePreference = { mode: "system" };
let current: ThemeDefinition = LIGHT_THEME;
let media: MediaQueryList | undefined;
/** React 渲染前应用主题，避免桌面窗口首帧闪烁。 */
export function initializeTheme(): void {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved && findTheme(saved)) preference = { mode: "theme", id: saved };
  media = window.matchMedia?.("(prefers-color-scheme: dark)");
  media?.addEventListener("change", apply);
  apply();
}
function apply(): void {
  current =
    preference.mode === "system"
      ? media?.matches
        ? DARK_THEME
        : LIGHT_THEME
      : (findTheme(preference.id) ?? LIGHT_THEME);
  const root = document.documentElement;
  root.dataset.theme = current.id;
  root.style.colorScheme = current.scheme;
  for (const [key, color] of Object.entries(current.colors))
    root.style.setProperty("--af-" + key, color);
  listeners.forEach((listener) => listener());
}
export function setTheme(id: string): void {
  if (id !== "system" && !findTheme(id)) throw new Error("主题不存在");
  preference = id === "system" ? { mode: "system" } : { mode: "theme", id };
  localStorage.setItem(STORAGE_KEY, id);
  apply();
}
export function selectedTheme(): string {
  return preference.mode === "system" ? "system" : preference.id;
}
export function activeTheme(): ThemeDefinition {
  return current;
}
export function subscribeTheme(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
export function useTheme(): ThemeDefinition {
  useSyncExternalStore(subscribeTheme, selectedTheme);
  return useSyncExternalStore(subscribeTheme, activeTheme);
}
