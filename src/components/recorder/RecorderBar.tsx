import { lazy, Suspense, useState } from "react";
import { createPortal } from "react-dom";
import { useRecorder } from "../../features/recorder";
import { RecorderBoundary } from "./RecorderBoundary";
import { RecorderMenu } from "./RecorderMenu";

/** 回看及视频时间线只在打开面板时加载，避免阻塞桌面首屏。 */
const RecorderPanel = lazy(() =>
  import("./RecorderPanel").then((module) => ({ default: module.RecorderPanel })),
);

/** 面板关闭时仍保留状态订阅，录制生命周期不依赖工作流标签。 */
function Controls() {
  const recorder = useRecorder();
  const [opened, setOpened] = useState(false);
  return (
    <>
      <RecorderMenu recorder={recorder} onOpen={() => setOpened(true)} />
      {opened &&
        createPortal(
          <Suspense fallback={null}>
            <RecorderPanel
              recorder={recorder}
              onClose={() => setOpened(false)}
            />
          </Suspense>,
          document.body,
        )}
    </>
  );
}

/** 始终挂载于标题栏的紧凑录制入口。 */
export function RecorderBar() {
  return (
    <RecorderBoundary>
      <Controls />
    </RecorderBoundary>
  );
}
