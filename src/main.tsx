import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import './styles.css'

const container = document.getElementById('root')
if (!container) {
  // 挂载点缺失说明 index.html 与入口不一致，此时静默失败会让界面变成白屏且无任何线索
  throw new Error('未找到 #root 挂载点：index.html 与应用入口不一致。')
}

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
