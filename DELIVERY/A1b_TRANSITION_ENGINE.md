# A1-b 分配/借用/移动/释放转移引擎 — 独立单元 fail-before / pass-after 交付

- 裁定：② 拆子项；本期只做五期中的第二期 A1-b（分配/借用/移动/释放四组转移）。
- 工作副本 `/content/oc-box-1-work/sars`，任务分支 `task/simpl-gaps-a1`，
  基线 = A1-a 工作树快照（载体 + 20 项真值表，无转移引擎），工作树交付。
- oracle：`/content/sa_all/sci/src/referee/table.zig:20-262`（`CapabilityTable`），
  真值表同源 `common/capability.zig:74-100`（A1-a 已落盘于 `mask.rs`）。

## 1. 本期范围（严格不越界）

新增 `crates/sa-verify/src/capability.rs`，把 sci `CapabilityTable` 的四个操作组
逐方法搬成 Rust：`note_alloc`（分配）、`start_read_borrow`/`start_mut_borrow`/
`release_borrow`（借用）、`note_move`/`note_move_view`（移动）、
`note_release_own`（释放），并保留 `masks`/`origins`/`lock_refs`/`flags` 四张同源
并行表与 `bind_active`/`bind_raw`/`note_read`/`note_write`/`finalize_leaks`。

`decide(prev_mask, op)` 是对 A1-a 真值表的**精确查表**访问器：表内 20 行逐行返回
`(legal, new_mask, trap)`，表外组合返回 `None`——无 if-else 近似、无回退泛化。
`pub mod capability` 由 `src/lib.rs:2` 导出。

**未动**：`rules.rs` 的 `transfer` 分派、`apply_cap`/`handle_call`（调用/FFI 规则）、
`state.rs` 的 `RegState` 裁决、phi 合并、诊断格式、任何 A2–I5 行为。本期是转移引擎
本体与逐条真值表证据，不改运行期分派口径；下游战绩一律不记入本期。

## 2. 完整语义口径（无简化）

- 引擎与 sci `table.zig` 逐方法对齐：借用视图 mask 按位合成为 `0x11`
  (`ACTIVE|BORROW_VIEW`)，FFI 源视图为 `0x33` (`ACTIVE|LOCKED_READ|BORROW_VIEW|FFI_BORROW`)
  **全四位**，未删位；`release_borrow` 带与 sci `clearBorrowCtx` 同源的
  borrow-view origin 守卫，避免把借用的借用误还原为 Active 造成假 `BorrowConflict`。
- 四类非法转移的 trap 与 sci 逐一对齐，而非沿用旧枚举机的近似分类：
  - 共享读后再可变借 → `ReadWriteConflict`（旧 `rules.rs` 报 `BorrowConflict`）；
  - 同一源二次可变借 → `DoubleMutableBorrow`；
  - 已消费后再分配/移动 → `UseAfterMove`；
  - 借出中移动/释放 → `BorrowConflict`。
- `decide` 对表外组合返回 `None`（例如 `0x02+Alloc`、`0x08+BorrowMut`），
  证明没有用近似规则补表。
- 口径边界：sci 自身的 `TRUTH_TABLE` 把 `0x08+Alloc` 记为非法 `UseAfterMove`，
  而 `CapabilityTable::noteAlloc` 允许从 consumed 重新分配（表注释自述为
  "spec/test parity"、verifier 用 imperative switch）。本期按 sci 双口径分别落盘：
  `decide` 忠实于表，`note_alloc` 忠实于 imperative 实现，并各自取证，不自造第三种口径。

## 3. 证据

| 项 | 结果 |
|---|---|
| fail-before（同一最终测试文件对 A1-a 快照树编译） | `rc=101`，`error[E0432]: unresolved import sa_verify::capability`，无 test result |
| pass-after（工作树） | `rc=0`，`test result: ok. 8 passed; 0 failed` |
| `cargo test --workspace` | `rc=0`，全部测试目标 0 failed |
| 968 名 `sars check`（A1-a 快照树 vs 本期工作树两棵 release 树） | 962 / 962，pass→fail=0，fail→pass=0 |
| 证据文件 | `DELIVERY/a1b_fail_before_pass_after.log`、`DELIVERY/a1b_harness.py` |
| 单元测试 | `crates/sa-verify/tests/a1b_transition_engine.rs`（8 项） |

fail-before 语义依据：A1-a 快照树只有载体与数据表，没有可执行 `decide`/四组转移方法，
测试在编译期即无法引用引擎 API；本次以最终测试文件的真实编译失败取证，非事后补写。
8 项测试覆盖：20 行逐行表驱动、表外无回退、分配/借用/移动/释放 sars 对齐、
二次可变借、共享→可变冲突、消费后移动、借出中移动、FFI 视图四位不简化。

## 4. 状态与交接

- 本期是独立单元，可单独验收；未借 A1-a 以外的任何战绩，未用简化语义冒充完整语义。
- A1-c（fallible/immutable/interior_ptr/owns_children 四位，对应 D1/B3/E1/B5）接期开工。
- 冻结面未触及：无回边、无 JS callback/float/codeval、无 81 sa_tmp、无 963/964。
