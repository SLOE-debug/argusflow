import { useState } from "react";
/** 只存界面偏好，不污染 workflow 执行定义。 */
export function usePanelSize(
  key: string,
  initial: number,
): readonly [number, (value: number) => void] {
  const [value, setValue] = useState(() => {
    const saved = Number(localStorage.getItem("argusflow.panel." + key));
    const bounds =
      key === "left" ? [180, 340] : key === "right" ? [260, 460] : [140, 600];
    return saved > 0 && Number.isFinite(saved)
      ? Math.max(bounds[0], Math.min(bounds[1], saved))
      : initial;
  });
  return [
    value,
    (next) => {
      setValue(next);
      localStorage.setItem("argusflow.panel." + key, String(next));
    },
  ];
}
export function usePanelVisible(
  key: string,
): readonly [boolean, (value: boolean) => void] {
  const [value, setValue] = useState(
    () => localStorage.getItem("argusflow.visible." + key) !== "false",
  );
  return [
    value,
    (next) => {
      setValue(next);
      localStorage.setItem("argusflow.visible." + key, String(next));
    },
  ];
}
