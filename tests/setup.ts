// 全局测试准备。
//
// 只做一件事：给 expect 补上 DOM 断言（toBeDisabled / toBeInTheDocument 等）。
// 这里**不** mock Tauri IPC —— 需要 mock 的测试在自己的文件里显式声明，
// 免得所有测试都在一个「假装 IPC 能用」的默认环境里跑。
import '@testing-library/jest-dom/vitest'
