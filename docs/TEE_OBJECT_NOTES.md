# Tee-Object 在 Windows PowerShell 下的真实行为（实测，非文档推断）

**结论先写**：对本项目的 CI 步骤，`Tee-Object` 是可用的；但它有一条容易被
误判的规则，必须记下来，否则下一次（无论是我还是别人）会得出错误的结论，
把一段本来正确的代码改坏。

## 规则

`cmd 2>&1 | Tee-Object -FilePath x` 会不会写出 `x`，
取决于**管道里实际有没有对象流过来**：

| 上游产生的是什么 | 流 | 写出文件？ |
|---|---|---|
| 原生 exe（`cargo` / `git`）的输出 | success stream | ✅ 会 |
| 同脚本里的 `Write-Output` / `Write-Host` | success stream | ✅ 会 |
| `& 另一个.ps1`，而那个脚本里用的是 **`Write-Host`** | **host stream** | ❌ **不会** |
| 目标文件的**父目录不存在** | — | ❌ 不会（且是静默的） |

`2>&1` 只把 **stderr 合并进 success stream**，它**不会**把别处的
`Write-Host` 拉进管道——`Write-Host` 走的是 host 流，本来就绕过管道。

## 证据

全部在 `C:\fp-ci-probe` 下跑出，PowerShell 5.1.26100.9444。

四组对照（同一台机器、同一种调用方式）：

```
=== A_WriteHost_callee_piped ===
   callee-line-1                  <- 内容打到了控制台
   callee-line-2
   A exists=False                 <- 文件没写
=== B_WriteOutput_callee_piped ===
   B exists=True                  <- Write-Output 走 success stream，写了
=== C_native_exe_piped ===
   C exists=True                  <- 原生 exe，写了
=== D_TeeObject_after_OutNull_missing ===
   D exists=False
```

直接对 `cargo` 验证（CI 步骤的真实形状）：

```
=== E.ps1 ===                      <- cargo --version 2>&1 | Tee-Object -FilePath e.log
   E exists=True bytes=76
   E captured=[cargo 1.98.1 (797e8a9bc 2026-08-05)]
```

**`cargo` 是原生 exe → 走 success stream → `Tee-Object` 正常写文件。**
`ci.yml` 里那一脚因此成立，保留原写法。

## 我差点得出的错误结论（这条比结论本身更值得记）

中途复刻 CI 步骤时，`Tee-Object` 没有写出日志。当时非常接近写下
「`Tee-Object` 在 PS 5.1 下不可靠，必须换掉」。

真实原因：我复刻时把被测命令换成了 `& fake-cargo.ps1`，
而那个假 cargo 内部用的是 `Write-Host` —— **是我的替身走错了流，
不是 `Tee-Object` 有问题。**

> **教训**：替身（stub/mock）改变了被测形状时，得到的结论是关于替身的，
> 不是关于系统的。在"管道能不能看见输出"这类问题上
> `Write-Host` 与 `Write-Output` **不等价**——写替身之前必须先确认它走哪条流。
> 这与「截图是线索，不是证据」同族：**替身是线索，不是证据。**

## 另一条真实的坑（与本项目直接相关，已写进 ci.yml）

`Tee-Object -FilePath` 指向一个**父目录不存在**的路径时：

- 静默不写文件；
- **退出码仍然是上游命令自己的**。

后果：日志在没有任何人察觉的情况下消失，而 CI 显示绿。
这正是 `ci.yml` 里加了「日志不存在就 `exit 1`」那条守卫的理由——
守卫已用替身演练验证会正确变红。

## 顺带踩到的环境坑（与结论无关，但很花时间）

- **PS 5.1 按 ANSI 码页读无 BOM 文件**：项目路径含中文目录（`开发`）时，
  任何"把路径写进生成脚本、再交给 `-File` 执行"的做法都会坏。
  实测 `-Encoding ascii` 把 `开发` 写成 `??`；mojibake 还会吃掉换行与
  收尾引号，报出看似无关的 `字符串缺少终止符`，把人引向错误的排查方向。
  → 这类实验要放在**纯 ASCII 路径**下做，且生成脚本一律 `-Encoding ascii`
  并保证内容不含非 ASCII。
- 这与项目记忆里 2026-09-25 记的「编辑 `.ps1` 之后要确认 BOM 还在」
  是**同一族**问题：PowerShell 5.1 读脚本时的编码判定。
