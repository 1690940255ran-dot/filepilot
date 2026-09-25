/**
 * (Re)generate `src/api/validators.generated.ts` —— 契约的**预编译**运行时校验器。
 *
 * ## 为什么必须预编译（这是一个只在安装版出现的发布阻断缺陷）
 *
 * 前端用 Ajv 在**运行时**把 `contracts.schema.json` 编译成校验函数，
 * 而 Ajv 默认走 `new Function(...)`。生产 CSP 是
 * `script-src 'self'`（不含 `'unsafe-eval'`），于是安装版里：
 *
 *     Uncaught EvalError: Evaluating a string as JavaScript violates the following
 *     Content Security Policy directive because 'unsafe-eval' is not an allowed
 *     source of script: script-src 'self' 'sha256-...'
 *
 * 校验器编译失败 → IPC 响应校验抛异常 → 前端启动即崩 → **窗口打开但整页白屏**。
 * 开发模式因为 `devCsp` 里带 `'unsafe-eval'`，一切正常——所以这个缺陷
 * **只在安装版出现**，而它正是 T17「干净机器验收」要抓的那类问题。
 *
 * 两条修法：
 *
 * 1. 给生产 CSP 加 `'unsafe-eval'` —— **拒掉**。那等于为了一个校验库
 *    把 XSS 的门开一条缝，是安全倒退。
 * 2. **构建期预编译**（本脚本）—— 保留运行时 schema 校验与严格 CSP，
 *    运行时不再有 `new Function`。
 *
 * ## 这个脚本产出什么
 *
 * Ajv 的 `standaloneCode()` 把已编译的校验函数**序列化成源码**，
 * 每个契约定义一个具名导出。生成物是普通 ES 模块，不含任何代码生成器。
 *
 * 用法（由 `pnpm contracts:generate` 调用）：
 *     node scripts/generate-validators.mjs [--check]
 *
 * `--check` 只比对、不写入：仓库里的生成物与当前 schema 不一致时以非零码退出。
 */

import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import Ajv from 'ajv'
import standaloneCode from 'ajv/dist/standalone/index.js'

const PROJECT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const SCHEMA_PATH = resolve(PROJECT, 'src/api/contracts.schema.json')
const OUTPUT_PATH = resolve(PROJECT, 'src/api/validators.generated.ts')

const checkOnly = process.argv.includes('--check')

const schema = JSON.parse(readFileSync(SCHEMA_PATH, 'utf8'))
const schemaId = schema.$id
if (!schemaId) {
    console.error('[validators] contracts.schema.json 缺少 $id')
    process.exit(1)
}

// 与原先运行时的 Ajv 选项逐条保持一致：换的是「何时编译」，不是「怎么校验」。
// 任何一项变了，行为就可能变——那属于契约变更，不属于构建方式变更。
const ajv = new Ajv({
    allErrors: true,
    strictSchema: false,
    validateFormats: false,
    // `lines: true`：默认生成物挤成一行（几百 KB 一个物理行），
    // 那样既没法 review、diff 也永远显示"整文件变更"。
    code: { source: true, esm: true, lines: true },
})
ajv.addSchema(schema, schemaId)

const definitionNames = Object.keys(schema.definitions ?? {})
if (definitionNames.length === 0) {
    console.error('[validators] schema 里没有 definitions')
    process.exit(1)
}

const refs = {}
for (const name of definitionNames) {
    refs[name] = `${schemaId}#/definitions/${name}`
}

const generated = standaloneCode(ajv, refs)

const banner = `// @ts-nocheck
/* eslint-disable */
//
// 本文件由 scripts/generate-validators.mjs 生成，**不要手改**。
//
// 为什么是预编译的：Ajv 运行时编译会 \`new Function()\`，而生产 CSP 不含
// 'unsafe-eval'（保留严格 CSP 是有意的），安装版会在启动时抛 EvalError 白屏。
// 预编译把「编译」从运行期挪到构建期，运行期只剩静态函数。
//
// 重新生成：pnpm contracts:generate

`

const output = `${banner}${generated}`

if (checkOnly) {
    let current = ''
    try {
        current = readFileSync(OUTPUT_PATH, 'utf8')
    } catch {
        console.error('[validators] 生成物不存在，请先运行 pnpm contracts:generate')
        process.exit(1)
    }
    if (current !== output) {
        console.error(
            '[validators] src/api/validators.generated.ts 与契约不一致。\n' +
            '  契约（Rust 类型 → JSON Schema）改过之后必须重新生成，否则\n' +
            '  前端校验的是旧形状。请运行：pnpm contracts:generate',
        )
        process.exit(1)
    }
    console.log(`[validators] 生成物与契约一致（${definitionNames.length} 个定义）`)
    process.exit(0)
}

writeFileSync(OUTPUT_PATH, output, 'utf8')
const sizeKb = (Buffer.byteLength(output) / 1024).toFixed(1)
console.log(
    `[validators] 已生成 src/api/validators.generated.ts：` +
    `${definitionNames.length} 个定义，${sizeKb} KB`,
)
