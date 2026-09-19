import { useId } from "react";
import { useAiSettings } from "../../features/ai";
import { Button, Checkbox, FormField, Input } from "../ui";
/** 只显示可编辑配置，已有密钥永远不回填。 */
export function AiSettings({
  settings,
  disabled,
}: {
  readonly settings: ReturnType<typeof useAiSettings>;
  readonly disabled: boolean;
}) {
  const id = useId();
  const { config } = settings;
  if (!config)
    return (
      <div className="space-y-3 text-sm text-muted">
        <p role={settings.error ? "alert" : "status"}>
          {settings.error ?? "正在读取 AI 配置…"}
        </p>
        {settings.error && (
          <Button onClick={() => void settings.load()}>重新读取</Button>
        )}
      </div>
    );
  const locked = disabled || settings.busy;
  const numbers = [
    ["max_rounds", "对话轮数", 1, 6],
    ["max_tools", "工具调用上限", 0, 12],
    ["max_tokens", "每轮输出 Token", 256, 16000],
    ["timeout_seconds", "请求超时（秒）", 10, 240],
  ] as const;
  return (
    <div className="space-y-3">
      <FormField label="接口地址" htmlFor={id + "-endpoint"}>
        <Input
          id={id + "-endpoint"}
          className="w-full"
          disabled={locked}
          value={config.endpoint}
          onChange={(e) =>
            settings.change({ ...config, endpoint: e.target.value })
          }
        />
      </FormField>
      <FormField label="模型名称" htmlFor={id + "-model"}>
        <Input
          id={id + "-model"}
          className="w-full"
          disabled={locked}
          value={config.model}
          onChange={(e) =>
            settings.change({ ...config, model: e.target.value })
          }
        />
      </FormField>
      <FormField label="API Key" htmlFor={id + "-key"}>
        <Input
          id={id + "-key"}
          type="password"
          autoComplete="off"
          className="w-full"
          disabled={locked}
          value={settings.key}
          placeholder={
            settings.view?.has_key ? "已设置；留空保留" : "输入 API Key"
          }
          onChange={(e) => settings.setKey(e.target.value)}
        />
      </FormField>
      <div className="grid grid-cols-2 gap-3">
        {numbers.map(([field, label, min, max]) => (
          <FormField key={field} label={label} htmlFor={id + field} stacked>
            <Input
              id={id + field}
              type="number"
              min={min}
              max={max}
              disabled={locked}
              value={config[field]}
              onChange={(e) =>
                settings.change({ ...config, [field]: Number(e.target.value) })
              }
            />
          </FormField>
        ))}
      </div>
      <div className="flex flex-wrap gap-4 text-xs">
        <Checkbox
          checked={config.vision}
          disabled={locked}
          label="允许发送开场缩略图和局部变化图"
          onCheckedChange={(vision) => settings.change({ ...config, vision })}
        />
        <Checkbox
          checked={config.bailian}
          disabled={locked}
          label="百炼接口（关闭深度思考）"
          onCheckedChange={(bailian) => settings.change({ ...config, bailian })}
        />
      </div>
      <p className="text-xs text-muted">
        配置与密钥保存在本机 SQLite。分析只使用已保存配置，不读取 .env。
      </p>
      {settings.error && (
        <p role="alert" className="text-xs text-danger">
          {settings.error}
        </p>
      )}
      {settings.saved && (
        <p role="status" className="text-xs text-muted">
          配置已保存
        </p>
      )}
      <div className="flex gap-2">
        <Button disabled={locked} onClick={() => void settings.save()}>
          {settings.busy ? "保存中…" : "保存配置"}
        </Button>
        <Button
          disabled={locked || !settings.view?.has_key}
          variant="ghost"
          onClick={() => void settings.save(true)}
        >
          清除密钥
        </Button>
      </div>
    </div>
  );
}
