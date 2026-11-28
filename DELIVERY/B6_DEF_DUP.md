# B6 同文件 #def 冲突诊断（sci parity）

- 日期：2026-11-28；批次父提交：6fd0721（B5 transfer，本地未推）
- oracle：sci `zig-out/bin/sa` + `src/flattener/def_dict.zig:41-67` putExpression
  （表达式先求值；同名不同值 → DuplicateDef；同值重复 → 容忍）

## fail-before / pass-after

- 用例：`/tmp/b6_audit/case1_conflict.sa`（`#def A = 1` 后 `#def A = 2`）。
- before（6fd0721 的 release 二进制）：sars `check` rc=0，静默通过；
  sci `check` rc=1，`error[DuplicateDef]`（trap_code 1002），定位第 2 行。
- after（本批）：sars `check` rc=1，
  `case1_conflict.sa:2:1 [E1002] DuplicateDef: duplicate definition detected during flattening`
  + `hint: conflicting definition 'A'`；与 sci 的 trap 名/码/报文/行号一致。
- 对照用例 `case2_identical.sa`（两处均为 `#def A = 1`）：sars 与 sci 均 rc=0，
  同值重复仍被容忍，无误伤。

## 改动

- `crates/sa-parser/src/parser.rs`：Parser 增 `def_values: HashMap<String, String>`，
  parse_program 内按文件初始化；`#def` 分支在推入 `program.defs` 前比较同名首值，
  不同值报 Trap::DuplicateDef（sci 同文案），相同值静默放行；每条重复仍保留在
  `program.defs`，与既有跨文件 imports::merge 的行为一致。
- 新增 2 个 parser 单测：不同值报告 + 同值容忍（含行号、文案与 hint 断言）。

## 门

- `cargo test --workspace`：41 套件，353 passed / 0 failed（新增 2 例）。
- `python3 .eval/sars_968.py target/release/sars`：total=968，
  rc=0 962 / sha256 830304a6…e2922；rc=1 6 / sha256 8f89c17e…d55，
  与 cf1a275 基线逐项一致，零翻转。
- 未推远端；按 #19312 本地 commit 报 muse 核验代推。
