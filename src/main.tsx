import React from "react";
import { createRoot } from "react-dom/client";
import { StudioApp } from "./components/workflow";
import { initializeTheme } from "./features/themes";
import "./styles.css";

/** HTML 入口固定提供根节点；缺失属于启动配置错误。 */
const root = document.getElementById("root");
if (!root) throw new Error("页面缺少 root 节点");
initializeTheme();
createRoot(root).render(
  <React.StrictMode>
    <StudioApp />
  </React.StrictMode>,
);
