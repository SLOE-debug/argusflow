import { Plus, Repeat2, Globe2, LoaderCircle, AlertCircle } from "lucide-react";
import { useStore } from "zustand";
import {
  studio,
  loopTemplate,
  browserTemplate,
} from "../../../features/workflow";
import { Button } from "../../ui";
export function Welcome() {
  const initialization = useStore(
    studio.store,
    (state) => state.initialization,
  );
  if (initialization.status !== "ready")
    return (
      <div className="flex min-w-0 flex-1 items-center justify-center bg-canvas p-8">
        {initialization.status === "failed" ? (
          <div className="max-w-lg space-y-3 text-center">
            <AlertCircle className="mx-auto text-danger" size={24} />
            <h1 className="text-base font-semibold">无法加载工作流数据</h1>
            <p
              role="alert"
              className="break-words text-xs leading-5 text-muted"
            >
              {initialization.error}
            </p>
            <Button
              onClick={() => {
                void studio.safely(() => studio.initializeWorkspace());
              }}
            >
              重试
            </Button>
          </div>
        ) : (
          <p
            role="status"
            className="flex items-center gap-2 text-xs text-muted"
          >
            <LoaderCircle size={16} className="animate-spin" />
            正在加载工作流…
          </p>
        )}
      </div>
    );
  return (
    <div className="flex flex-1 items-center justify-center bg-canvas">
      <div className="w-[520px] p-8">
        <div className="mb-6 flex items-center gap-3">
          <img src="/icon.png" alt="" className="size-12" />
          <div>
            <h1 className="text-xl font-semibold tracking-tight">
              把步骤连接起来
            </h1>
            <p className="mt-1 text-xs text-muted">
              搭建、配置、运行，在同一个工作区完成。
            </p>
          </div>
        </div>
        <div className="flex gap-2">
          <Button
            variant="primary"
            onClick={() => {
              void studio.safely(() => studio.create());
            }}
          >
            <Plus size={14} />
            新建工作流
          </Button>
        </div>
        <div className="mt-8 border-t border-line pt-5">
          <h2 className="mb-3 text-xs text-muted">从示例开始</h2>
          <div className="grid grid-cols-2 gap-3">
            <Button
              className="h-20 justify-start gap-3 px-4"
              onClick={() => studio.create(loopTemplate())}
            >
              <Repeat2 size={20} className="text-structure" />
              <span className="text-left">
                <span className="block">循环与变量</span>
                <span className="mt-1 block text-[10px] font-normal text-muted">
                  遍历列表，累计并返回结果
                </span>
              </span>
            </Button>
            <Button
              className="h-20 justify-start gap-3 px-4"
              onClick={() => studio.create(browserTemplate())}
            >
              <Globe2 size={20} className="text-accent" />
              <span className="text-left">
                <span className="block">浏览器操作</span>
                <span className="mt-1 block text-[10px] font-normal text-muted">
                  打开页面，定位并操作元素
                </span>
              </span>
            </Button>
          </div>
        </div>
        <p className="mt-6 text-[11px] leading-6 text-muted">
          工作流自动保存在本机，关闭后可继续编辑。
          <br />
          拖入节点，Tab 搜索，Ctrl+C / Ctrl+V 复用步骤。
        </p>
      </div>
    </div>
  );
}
