import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

interface Target {
  value: string | null;
  password: boolean;
  context: string[];
}
interface Snapshot {
  lost: number;
  events: { kind: string; input_type: string; target: Target }[];
}
interface Observer {
  take(): Snapshot;
  stop(): void;
}
const script = readFileSync(
  resolve("crates/argusflow-browser/src/page/observation.js"),
  "utf8",
);
const observer = () =>
  (globalThis as typeof globalThis & { __argusflowRecorderV1?: Observer })
    .__argusflowRecorderV1;
const install = () => {
  window.eval(script);
};
afterEach(() => {
  observer()?.stop();
  document.body.replaceChildren();
});

describe("被动页面监听（JSDOM，不启动浏览器）", () => {
  it("重复安装不重复事件，读取清空队列，停止后卸载", () => {
    install();
    install();
    const button = document.createElement("button");
    document.body.append(button);
    button.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(observer()?.take().events).toHaveLength(1);
    expect(observer()?.take().events).toHaveLength(0);
    observer()?.stop();
    expect(observer()).toBeUndefined();
    install();
    expect(observer()?.take().events).toHaveLength(0);
  });
  it("有界事件溢出保留已知缺口，不把后续事件当完整", () => {
    install();
    for (let i = 0; i < 140; i++)
      document.body.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    const snapshot = observer()?.take();
    expect(snapshot?.events).toHaveLength(128);
    expect(snapshot?.lost).toBe(12);
    expect(observer()?.take().lost).toBe(0);
  });
  it("真实编辑值保留 inputType，密码不泄露，开放 Shadow 保留边界", () => {
    install();
    const host = document.createElement("div");
    document.body.append(host);
    const root = host.attachShadow({ mode: "open" });
    const input = document.createElement("input");
    root.append(input);
    input.value = "中文粘贴";
    input.dispatchEvent(
      new InputEvent("input", {
        bubbles: true,
        composed: true,
        inputType: "insertFromPaste",
      }),
    );
    const [event] = observer()!.take().events;
    expect(event.input_type).toBe("insertFromPaste");
    expect(event.target.value).toBe("中文粘贴");
    expect(event.target.context).toContain("open-shadow-root");
    input.type = "password";
    input.value = "private-test-value";
    input.dispatchEvent(
      new InputEvent("input", {
        bubbles: true,
        composed: true,
        inputType: "insertCompositionText",
      }),
    );
    const secret = observer()!.take();
    expect(secret.events[0].target.password).toBe(true);
    expect(secret.events[0].target.value).toBeNull();
    expect(JSON.stringify(secret)).not.toContain("private-test-value");
  });
});
