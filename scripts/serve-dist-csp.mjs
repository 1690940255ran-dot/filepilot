/**
 * 用**生产 CSP** 静态托管 `dist/`，供 Playwright 回归用例使用。
 *
 * ## 为什么需要它
 *
 * 安装版曾经整页白屏，原因是生产 CSP 不含 `'unsafe-eval'`，而前端用 Ajv 在
 * 运行时编译 schema（`new Function`）→ 校验器编译抛 `EvalError` → 前端启动即崩。
 *
 * 这个缺陷**逃过了所有既有测试**：`pnpm dev` 与 `pnpm test:e2e` 走的都是
 * `devCsp`（里面带 `'unsafe-eval'`），所以本地怎么跑都是好的。
 *
 * 这个服务器把 `tauri.conf.json` 里那条**生产 CSP**原样作为响应头下发，
 * 于是「生产 CSP 下前端能否启动」变成一条可回归的用例。
 *
 * **边界**：它验证的是 CSP 语义，不是安装包本身（打包、安装、路径解析
 * 三层仍要真机验收，见 `docs/CLEAN_MACHINE_ACCEPTANCE.md`）。
 */

import { readFileSync, existsSync, statSync } from 'node:fs'
import { createServer } from 'node:http'
import { dirname, extname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const PROJECT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const DIST = resolve(PROJECT, 'dist')
const PORT = Number(process.env.PROD_CSP_PORT ?? 1431)

const tauriConfig = JSON.parse(
    readFileSync(resolve(PROJECT, 'src-tauri/tauri.conf.json'), 'utf8'),
)
// 只取**生产** CSP：`devCsp` 不是发出去的那一条，用它测等于没测
const productionCsp = tauriConfig.app.security.csp
if (!productionCsp) {
    console.error('[prod-csp] tauri.conf.json 里没有 app.security.csp，无法复现生产环境')
    process.exit(1)
}

const MIME = {
    '.html': 'text/html; charset=utf-8',
    '.js': 'text/javascript; charset=utf-8',
    '.css': 'text/css; charset=utf-8',
    '.json': 'application/json; charset=utf-8',
    '.map': 'application/json; charset=utf-8',
    '.svg': 'image/svg+xml',
    '.png': 'image/png',
    '.ico': 'image/x-icon',
}

const server = createServer((req, res) => {
    const urlPath = decodeURIComponent((req.url ?? '/').split('?')[0])
    const relative = urlPath === '/' ? 'index.html' : urlPath.replace(/^\/+/, '')
    const target = join(DIST, relative)

    // 目录穿越保护（测试服务器也要有基本底线）
    if (!target.startsWith(DIST)) {
        res.writeHead(403).end('forbidden')
        return
    }
    if (!existsSync(target) || !statSync(target).isFile()) {
        res.writeHead(404, { 'Content-Security-Policy': productionCsp }).end('not found')
        return
    }

    res.writeHead(200, {
        'Content-Type': MIME[extname(target)] ?? 'application/octet-stream',
        // ← 这一行就是这个脚本存在的理由
        'Content-Security-Policy': productionCsp,
    })
    res.end(readFileSync(target))
})

server.listen(PORT, '127.0.0.1', () => {
    console.log(`[prod-csp] dist/ 已在 http://127.0.0.1:${PORT} 上以生产 CSP 托管`)
    console.log(`[prod-csp] CSP: ${productionCsp.slice(0, 80)}…`)
})
