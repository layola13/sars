# C(native) 簇交付（接 42704ea extern，回主会话 07:25 派单的第二项）

- 口径：一类一提交、fail-before/pass-after、pass→fail=0、禁简化；只追
  `build_exe_fail_classify.json` 中 `native` 标签的 1 名（117 inline asm）。
- before = `42704ea`（extern 提交，工作树干净）；
  after = 任务分支 `task/native-c-backend-42704ea` 本提交。
- 工作副本：`/content/oc-box-1-work/sars`（一人一份；`../sci` 软链到 `/content/sa_all/sci`）。
- 改动：`crates/sa-emit-c/src/{emit.rs,tests.rs}` + `DELIVERY/c_native_fail_before_pass_after.log`。

## 根因与落地形态

`emit_instruction()` 原把 `InstructionKind::Native { .. }` 与 va_list  intrinsics
合并，统一发 `[E2002] UnsupportedInstruction: variadic / native intrinsics`，故
117 在 `build-exe` 阶段即失败。本笔拆开两类：

1. **native**：`inline_asm()` 从 IR 文本 `call void asm sideeffect "", ""()` 取
   asm body 与 `sideeffect` 限定符，生成
   `__asm__ __volatile__("<body>" ::: "memory");`（无 sideeffect 则发 `__asm__("<body>")`）；
   body 里的 `\`/`"`/换行按 C 字符串转义。无法解析时仍发诊断，不静默。
2. **va_list**：`VaStart/VaArg/VaEnd` 保留原拒收口径，避免误放行变参 intrinsics。

117 的 `$call void asm sideeffect "", ""()$` 落为带 memory clobber 的空 asm，
`dump-c` 中可见 `__asm__ __volatile__("" ::: "memory");`，store→load 的 7 不被
副作用 asm 破坏。

## fail-before / pass-after

复现件：`DELIVERY/c_extern_harness.py`（968 名，before/after 两二进制各自
`build-exe` + 实跑，记录 rc 与逐字 stdout），日志
`DELIVERY/c_native_fail_before_pass_after.log`。

| 指标 | before(42704ea) | after(本笔) |
|---|---|---|
| 全语料 | 968 | 968 |
| rc=0（build+run 双绿） | 930 | **931** |
| native 1 名 rc=0 | 0 | **1** |
| extern 28 名 rc=0 | 28 | 28 |
| vtable 112 名 rc=0 | 112 | 112 |
| pass → fail | — | **0** |
| stdout 回归（原本双绿者逐字比对） | — | **0** |

- 117 逐字 stdout 命中 `7\n`（日志 `before_rc=1 after_build_rc=0 run_rc=0 stdout='7\n'`）。
- before 二进制（extern 笔产物）sha256 =
  `72cca8dec063a70588c69233ce10892fde065be3a54444ddfa37bc7b42bffdc6`；
  after 二进制 sha256 =
  `60f88d5e55deada5474834a1e8c34b571e630f9812ba075733ad42629177c9c3`。

## 验证

- `cargo test --workspace` 全绿（新增 `emits_sideeffect_inline_asm`，
  断言 `__asm__ __volatile__("" ::: "memory")` 形态且无残留拒收诊断）。
- 取证件 sha256：
  - `DELIVERY/c_native_fail_before_pass_after.log` = `3dca729b740d1fb4656f8f303b4e4befc13d95a0bb030694f9d89a34282e35db`
  - `DELIVERY/c_extern_harness.py` = `8780e5ae474d758b3132cfb086c5cf86cf2232ed4b1ba32e7b6c8d2062eefdf1`
  - `crates/sa-emit-c/src/emit.rs` = `9c1dc51b5e7ab1ded2cca06a323edcb1cf9c0bd25c7d0f607ee5322f2fb1a161`
  - `crates/sa-emit-c/src/tests.rs` = `cd4aa2981cb8de23d4cb94a7e7e2f19bfbad7f02d6f49abe14cf98014137370a`
  - `DELIVERY/C_NATIVE.md`（本文 sha 见中继回报，避免自引用）

## 交付形态与推送

工作分支 `task/extern-c-backend-24a2481`（42704ea）与
`task/native-c-backend-42704ea`（本笔）只留本地；交付经主会话既有 main 推。
推送按知识库第 1/3 条：盒上 `/proc/<pid>/environ` 取 `GIT_TOKEN`，只经
`git -c http.extraheader=...` 传给 git，不回显/不落盘/不进配置；
推前 `git ls-remote` 核旧值，推后核新值；不建远端分支、不用浏览器自动化。
