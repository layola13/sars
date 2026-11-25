# B 簇交付：#npm_bind 跨后端门（回 10860 · 10 名诊断对齐）

- 排序：D→E→C(wasm)→C(JS)→F→**B**；本笔只做 B，不并簇。
- 裁定依据：10695 簇 B 清单 + 10700 ①「#npm_bind 补门后按诊断对齐验收——sars 与 sci 同诊断类
  响亮拒绝即 parity，不要求 rc 翻绿」。before HEAD = `a4bb214` 快照 `/tmp/opencode/b_before/sars`。

## 根因与对齐形态

sci `src/cli/compile.zig:328-335` 的 `rejectNpmBindsUnlessJs("sa build-wasm", ...)` 对任何带
`#npm_bind` 的程序具名拒（`NpmImportJsOnly`）；`src/cli/build_wasm.zig:80` 在发射前调用。
sars `commands.rs::build_wasm` 原先无此门（与 `build_exe` 的 D22 门不对齐），程序照常发射，
宿主零桩使 10 名运行期 rc=1 / `error`，属「静默错结果」而非「显式拒」。

落地：`build_wasm` 在 `emit_wasm` 之前检查 `compilation.program.npm_binds`，非空即发
`Trap::UnsupportedInstruction` 具名诊断
`build-wasm rejects #npm_bind programs (D22): npm bindings are JS-only`，整笔 `fail()` 返回 1。
与 `build_exe` 的 D22 门同诊断类、同判据；**不改 JS 路径、不改 emitter、不改 verifier**。
新增单测 `commands::tests::build_wasm_rejects_npm_bind`（rc=1 且不产 wasm）。

## 诊断对齐（10 名逐名，sci vs sars）

| 名 | sci build-wasm | sars build-wasm | 判读 |
|---|---|---|---|
| 496_npm_os_totalmem / 497_npm_os_parallelism / 498_npm_path_isabs | rc=1，含 `#npm_bind` 与 JS-only | rc=1，E2002 UnsupportedInstruction，含 `#npm_bind` / JS-only | 同诊断类 |
| 499_npm_util_isequal / 500_npm_ns_isabs / 501_npm_ns_isequal | 同上 | 同上 | 同上 |
| 965_npm_handle_ctor_method / 966_npm_handle_string_out / 967_npm_handle_bytes_view / 968_npm_handle_callback | 同上 | 同上 | 同上 |

sci 原文：`error: sa build-wasm does not support \`#npm_bind\` npm imports; use \`sa build-js\` (JS target only)`。
10/10 `aligned=True`，逐名全文见 `DELIVERY/b_fail_before_pass_after.log` 的 `== DIAGNOSTIC ALIGNMENT ==` 段。

## 全语料与 JS 路径

同一 typed-stub runner（`mode="c"`），before/after 唯一变量是本笔的门。

| 指标 | before | after |
|---|---|---|
| 969 wasm rc=0 | 927 | **927**（不变） |
| 10 名 npm 运行期结果 | 失败（零桩 rc=1） | 失败（构建期显式拒 rc=1） |
| pass → fail | — | **0** |
| 原通过名 stdout 变化 | — | **0** |

JS 路径逐名复查：10 名 `build-js` + node 仍 rc=0，stdout `496\n`/`497\n`/…/`968\n` 逐字不变
（见日志 `== JS PATH UNAFFECTED ==` 段），门未外溢到 `build-js`。
`cargo test --workspace` **212/212** 全绿（原 211 + 新增 1）。

## 证据

- `crates/sa-cli/src/commands.rs:188-205`、`:335-357`（单测）
- `DELIVERY/b_harness.py`、`DELIVERY/b_fail_before_pass_after.log`

## 后续

10695 的 A/B/C/D/E/F/G 簇全部落地结案。build-exe 逐类扩面（fallible ABI / vtable / atomics）
按 10700 ④ 另簇推进。本笔本地提交后请主会话代推并 PAT 核验，不停线。
