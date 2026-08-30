import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import AppBootstrap from './AppBootstrap';
import './styles.css';

/** 将 React 应用挂载到 index.html 提供的 root 容器。 */
createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <AppBootstrap />
  </StrictMode>,
);
