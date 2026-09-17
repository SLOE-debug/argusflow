import { THEME_TOKENS, type ThemeDefinition } from "./model";

/** 浅色工作台使用中性白底，彩色仅用于操作与语义标记。 */
export const LIGHT_THEME: ThemeDefinition = {
  id: "light",
  name: "浅色",
  scheme: "light",
  colors: {
    app: "#fafafa",
    panel: "#ffffff",
    surface: "#ffffff",
    subtle: "#f5f5f5",
    hover: "#eeeeee",
    canvas: "#fafafa",
    grid: "#a3a3a3",
    snapGuide: "#a044df",
    border: "#e5e5e5",
    strong: "#a3a3a3",
    text: "#202020",
    muted: "#707070",
    accent: "#2563eb",
    accentHover: "#1d4ed8",
    accentSoft: "#eff4ff",
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
    edge: "#8a8a8a",
  },
  syntax: {
    keyword: "2563EB",
    string: "B45309",
    number: "7C3AED",
    comment: "707070",
  },
};
/** 深色工作台以炭黑分层，避免蓝灰底色影响内容辨识。 */
export const DARK_THEME: ThemeDefinition = {
  id: "dark",
  name: "深色",
  scheme: "dark",
  colors: {
    app: "#171717",
    panel: "#1c1c1c",
    surface: "#242424",
    subtle: "#292929",
    hover: "#333333",
    canvas: "#171717",
    grid: "#666666",
    snapGuide: "#cc91ff",
    border: "#353535",
    strong: "#666666",
    text: "#ededed",
    muted: "#a3a3a3",
    accent: "#609bff",
    accentHover: "#8ab5ff",
    accentSoft: "#25334a",
    onAccent: "#171717",
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
    edge: "#858585",
  },
  syntax: {
    keyword: "80B2FF",
    string: "ECC48D",
    number: "C4A2FF",
    comment: "A3A3A3",
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
