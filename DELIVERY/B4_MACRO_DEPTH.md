# B4 宏展开深度对齐 sci（sci parity）

- 日期：2026-11-28；批次父提交：285073d（B6）
- oracle：sci `src/flattener/importexpansion.zig:13-16`（max_macro_recursion_depth=256）
  与 `src/flattener/emitrange.zig:213`（depth > 256 → MacroRecursionLimit）

## fail-before / pass-after

- 用例 `/tmp/b4_audit/case_chain40b.sa`：40 个不同名宏逐级 `EXPAND` 链，末端一条有效指令。
  - before（285073d 的 release 二进制）：sars rc=1，自第 33 层起报 E1005 MacroRecursionLimit；
    sci rc=0（256 层以内合法）。
  - after：sars rc=0，与 sci 一致。
- 阈值边界双边实测：256 层链双方 rc=0；257 层链双方 rc=1。
  sars 报 `[E1005] MacroRecursionLimit: macro recursion limit exceeded`，
  sci 报 `error[MacroRecursionLimit]: macro recursion limit exceeded`（trap 名/码/报文一致；
  sars 的 SourceLoc 仍是 unknown，属既有定位精度缺口，不在本批范围）。
- 自递归用例 `/tmp/b4_audit/case_cycle.sa`：双方均 rc=1 同报文，无误伤。

## 改动

- `crates/sa-flatten/src/lib.rs`：FlattenOptions::default max_depth 32 → 256。
  `expand_call` 在 depth>=max_depth 时报错，首层为 depth=0，故 256 恰好放行 256 层、拒绝第 257 层，
  与 sci 的 `depth > 256` 同阈值。
- `crates/sa-flatten/src/macros.rs`：两处 MacroRecursionLimit 报文改为 sci 固定文案
  `macro recursion limit exceeded`；新增深链展开单测（40 层无误报）。
- 默认配置单测断言同步为 256。

## 门

- `cargo test --workspace`：41 套件，354 passed / 0 failed（新增 1 例）。
- `python3 .eval/sars_968.py target/release/sars`：total=968，
  rc=0 962 / sha256 830304a6…e2922；rc=1 6 / sha256 8f89c17e…d55，
  与 cf1a275 基线逐项一致，零翻转。

## 余项登记

- sci 还有全局 `max_macro_expansion_events=10M`、`max_expanded_macro_lines=10M` 与
  `max_expanded_instructions=10M` 三道预算；sars 仍只有局部深度/环检测。
  本批只对齐可源码复现的深度阈值，全局预算无干净 fail-before（构造 10M 级语料不现实），
  按证据纪律不硬造，登记待裁。
- 本地 commit，未推；按 #19312 报 muse 核验代推。
