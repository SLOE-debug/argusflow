import { Moon, Sun } from "lucide-react";
import { useTheme, setTheme } from "../../features/themes";
import { IconButton } from "../ui";

/** 应用外观偏好入口，独立于工作流操作。 */
export function ThemePicker() {
  const theme = useTheme();
  const isDark = theme.scheme === "dark";
  return (
    <IconButton
      aria-label={isDark ? "切换到浅色" : "切换到深色"}
      className="size-6 text-ink"
      onClick={() => setTheme(isDark ? "light" : "dark")}
    >
      {isDark ? (
        <Sun size={14} aria-hidden="true" />
      ) : (
        <Moon size={14} aria-hidden="true" />
      )}
    </IconButton>
  );
}
