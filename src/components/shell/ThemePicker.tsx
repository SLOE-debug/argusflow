import { SunMoon } from "lucide-react";
import {
  useTheme,
  themeCatalog,
  selectedTheme,
  setTheme,
} from "../../features/themes";
import { Select } from "../ui";

/** 应用外观偏好入口，独立于工作流操作。 */
export function ThemePicker() {
  useTheme();
  return (
    <div className="flex shrink-0 items-center gap-1">
      <SunMoon size={13} aria-hidden="true" />
      <Select
        aria-label="主题"
        className="h-6 w-24 border-transparent bg-transparent text-[11px] shadow-none"
        value={selectedTheme()}
        onValueChange={setTheme}
        options={[
          { value: "system", label: "跟随系统" },
          ...themeCatalog().map((theme) => ({
            value: theme.id,
            label: theme.name,
          })),
        ]}
      />
    </div>
  );
}
