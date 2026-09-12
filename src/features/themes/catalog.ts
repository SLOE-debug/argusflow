import { THEME_TOKENS, type ThemeDefinition } from "./model";

/** 浅色专业工作台。 */
export const LIGHT_THEME: ThemeDefinition = {
  id: "light",
  name: "浅色",
  scheme: "light",
  colors: {
    app: "#f3f5f9",
    panel: "#f8fafc",
    surface: "#ffffff",
    subtle: "#f1f5f9",
    hover: "#e9eef6",
    canvas: "#f5f8fc",
    grid: "#91a7c3",
    snapGuide: "#a044df",
    border: "#e0e6ef",
    strong: "#bbc8d9",
    text: "#1b283c",
    muted: "#69788c",
    accent: "#2563eb",
    accentHover: "#1d4ed8",
    accentSoft: "#eaf1ff",
    onAccent: "#ffffff",
    success: "#16834c",
    successSoft: "#e5f6ed",
    danger: "#d13c54",
    dangerSoft: "#fff0f2",
    nodeStart: "#26978b",
    nodeEnd: "#cf6884",
    warning: "#b97516",
    structure: "#8560d8",
    structureSoft: "#f3effc",
    data: "#16834c",
    browser: "#2563eb",
    desktop: "#b97516",
    edge: "#93a4bb",
  },
  syntax: {
    keyword: "2563EB",
    string: "B45309",
    number: "7C3AED",
    comment: "64748B",
  },
};
/** 深色主题沿用同一语义和布局。 */
export const DARK_THEME: ThemeDefinition = {
  id: "dark",
  name: "深色",
  scheme: "dark",
  colors: {
    app: "#111722",
    panel: "#18212e",
    surface: "#1d2838",
    subtle: "#202d40",
    hover: "#2a3a50",
    canvas: "#141c29",
    grid: "#536d90",
    snapGuide: "#cc91ff",
    border: "#2c3a4e",
    strong: "#4b607b",
    text: "#e4ecf8",
    muted: "#93a5be",
    accent: "#609bff",
    accentHover: "#8ab5ff",
    accentSoft: "#233a60",
    onAccent: "#101d33",
    success: "#59ce92",
    successSoft: "#193e32",
    danger: "#ff8497",
    dangerSoft: "#452633",
    nodeStart: "#6bc9bb",
    nodeEnd: "#e89bb2",
    warning: "#edbc68",
    structure: "#ae91fa",
    structureSoft: "#2b2642",
    data: "#59ce92",
    browser: "#609bff",
    desktop: "#edbc68",
    edge: "#637d9e",
  },
  syntax: {
    keyword: "80B2FF",
    string: "ECC48D",
    number: "C4A2FF",
    comment: "93A5BE",
  },
};
const themes = new Map<string, ThemeDefinition>([
  [LIGHT_THEME.id, LIGHT_THEME],
  [DARK_THEME.id, DARK_THEME],
]);
/** 应用装配时注册新主题，拒绝重复身份及不完整令牌。 */
export function registerTheme(theme: ThemeDefinition): void {
  if (themes.has(theme.id) || theme.id === "system")
    throw new Error("主题标识重复");
  if (THEME_TOKENS.some((key) => !theme.colors[key]))
    throw new Error("主题令牌不完整");
  themes.set(theme.id, theme);
}
export function themeCatalog(): readonly ThemeDefinition[] {
  return [...themes.values()];
}
export function findTheme(id: string): ThemeDefinition | undefined {
  return themes.get(id);
}
