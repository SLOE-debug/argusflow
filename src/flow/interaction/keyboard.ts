/** 文本输入、输入法、编辑器和菜单保留自身键盘行为。 */
export function ownsKeyboard(event: KeyboardEvent): boolean {
  if (event.isComposing) return true;
  return (
    event.target instanceof Element &&
    Boolean(
      event.target.closest(
        "input,textarea,select,dialog,[contenteditable=true],.monaco-editor,[role=dialog],[role=menu],[role=combobox],[role=checkbox],[role=radio],[role=switch],[role=tree]",
      ),
    )
  );
}
export function hasTextSelection(): boolean {
  const selection = window.getSelection();
  return Boolean(selection && !selection.isCollapsed);
}
