# A1-c fallible / immutable / interior_ptr / owns_children 位 — 独立单元 fail-before / pass-after 交付

- 裁定：② 拆子项；本期只做五期中的第三期 A1-c（四个派生位）。
- 工作副本 `/content/oc-box-1-work/sars`，任务分支 `task/simpl-gaps-a1`，
  基线 = A1-b 工作树快照（四组转移引擎，无派生位方法），工作树交付。
- oracle：`common/capability.zig:9-18` 位定义、`74-100` 真值表 15/16/17/19 行，
  `referee/table.zig:172-243` 的借用/释放守卫。

## 1. 本期范围（严格不越界）

在 `capability.rs` 增加四个派生位方法：`bind_fallible`/`resolve_fallible`、
`bind_immutable`、`note_interior_ptr`、`transfer_own`；并让 `note_alloc` 对 immutable
保位、`note_move` 对 interior_ptr 保位（与真值表 15/16 行逐项对齐）。
`EngineError` 增加 `TransferIntoBorrow` / `TransferIntoImmutable` 两个精确 trap 变体。

**未动**：`rules.rs` 的 `transfer`/`apply_cap`/`handle_call`、`state.rs`、`lib.rs`
的模块结构（A1-b 已完成导出）、phi 合并、诊断格式、任何下游 D1/B3/E1/B5 战绩。

## 2. 逐位完整语义（无简化）

| 位 | 方法 | 语义 | 真值表锚 |
|---|---|---|---|
| `0x080 fallible` | `bind_fallible` / `resolve_fallible` | 置位/清位，不触碰其他位 | 载体 A1-a 已断言专名 |
| `0x100 immutable` | `bind_immutable` | `note_alloc` 保位（`0x100+Alloc→0x100`）；`0x101` 上 `transfer_own` 非法 | 15、19 行 |
| `0x200 interior_ptr` | `note_interior_ptr` | `note_move` 保位（`0x200+Move→0x200`） | 16 行 |
| `0x400 owns_children` | `transfer_own` | 源按 `0x01+TransferOwn→0x08` 退役；目的置 `OWNS_CHILDREN`，借视图/immutable 目的拒绝 | 17、18、19 行 |

`transfer_own` 的目的检查先做位级守卫（BORROW_VIEW → `TransferIntoBorrow`，IMMUTABLE →
`TransferIntoImmutable`），再走 `decide` 的精确表项；不是 if-else 近似真值表，而是
sci 的 imperative 目的检查 + 表驱动源退役双口径。复合态 `0x201` 的
`mask_components` 同时列出 `Active`/`Fallible`/`Immutable`/`InteriorPtr`，不删位。

## 3. 证据

| 项 | 结果 |
|---|---|
| fail-before（同一最终测试文件对 A1-b 快照树编译） | `rc=101`，`E0599: no method named bind_fallible`，无 test result |
| pass-after（工作树） | `rc=0`，`test result: ok. 6 passed; 0 failed` |
| `cargo test --workspace` | `rc=0`，全部测试目标 0 failed |
| 968 名 `sars check`（A1-b 快照树 vs 本期） | 962 / 962，pass→fail=0，fail→pass=0 |
| 证据文件 | `DELIVERY/a1c_fail_before_pass_after.log`、`DELIVERY/a1c_harness.py` |
| 单元测试 | `crates/sa-verify/tests/a1c_derived_bits.rs`（6 项，逐位独立） |

六项测试分别独立验收：fallible 置/清、immutable 保位+拒绝 transfer、interior_ptr
保位、owns_children 由 transfer 置位、转入借视图拒绝、四位互不干扰。

## 4. 状态与交接

- 本期独立单元，逐位 fail-before/pass-after；未借 D1/B3/E1/B5 战绩，未用简化语义。
- A1-d（合并格：带 mask 的 label 快照 + 分支冲突 trap，对应 A2/A3）接期开工。
- 冻结面未触及：无回边、无 JS callback/float/codeval、无 81 sa_tmp、无 963/964。
