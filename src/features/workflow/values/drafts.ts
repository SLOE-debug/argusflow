/** 复合字段保存每项的未完成原文，修复其他项不会误清除错误。 */
export function readCompoundDraft(
  source?: string,
): Readonly<Record<string, string>> {
  if (!source) return {};
  try {
    const parsed: unknown = JSON.parse(source);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed))
      return {};
    const entries = Object.entries(parsed);
    return Object.fromEntries(
      entries.filter(
        (entry): entry is [string, string] => typeof entry[1] === "string",
      ),
    );
  } catch {
    return {};
  }
}
