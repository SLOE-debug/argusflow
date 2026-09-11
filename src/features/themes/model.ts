/** 主题的完整语义令牌，组件不依赖具体配色。 */
export const THEME_TOKENS = [
  "app",
  "panel",
  "surface",
  "subtle",
  "hover",
  "canvas",
  "grid",
  "border",
  "strong",
  "text",
  "muted",
  "accent",
  "accentHover",
  "accentSoft",
  "onAccent",
  "success",
  "successSoft",
  "danger",
  "dangerSoft",
  "warning",
  "structure",
  "structureSoft",
  "data",
  "browser",
  "desktop",
  "edge",
] as const;
export type ThemeToken = (typeof THEME_TOKENS)[number];
/** 注册主题必须同时提供 WebView 和 Monaco 配色。 */
export interface ThemeDefinition {
  readonly id: string;
  readonly name: string;
  readonly scheme: "light" | "dark";
  readonly colors: Readonly<Record<ThemeToken, string>>;
  readonly syntax: {
    readonly keyword: string;
    readonly string: string;
    readonly number: string;
    readonly comment: string;
  };
}
export type ThemePreference =
  { readonly mode: "system" } | { readonly mode: "theme"; readonly id: string };
