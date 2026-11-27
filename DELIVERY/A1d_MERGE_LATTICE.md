# A1-d 合并格 — 独立单元 fail-before / pass-after 交付

- 裁定：② 拆子项；本期只做五期中的第四期 A1-d（带 mask 的 label 快照 + 分支冲突 trap）。
- 工作副本 `/content/oc-box-1-work/sars`，任务分支 `task/simpl-gaps-a1`，
  基线 = A1-c 工作树快照（无合并格 / 快照 API），工作树交付。
- oracle：`lib.rs:105-130 phi_warning`（当前分支冲突仅 warning）、`state.rs:53-116 merge`；
  trap 分类沿用 sci `capability.zig:61-72` 的 `TrapKind`。

## 1. 本期范围（严格不越界）

新增 `crates/sa-verify/src/merge.rs` 并由 `lib.rs:5` 导出：`MaskSnapshot`（masks/origins/
lock_refs/flags 四表快照）、`MergeTrap`、`MergeConflict`、`merge_snapshots`；
`CapabilityEngine::snapshot()/restore()` 负责 label 快照落盘/恢复。

`join_masks` 用位级格规则而非分支 if-else 近似：相等直接保留；读锁/写锁相撞 →
`ReadWriteConflict` trap；consumed 与非 consumed 相撞 → `UseAfterMove` trap；借用位
相异 → `BorrowConflict` trap；其余位差异 → `Divergence`（join 取按位或）。
冲突以 `MergeConflict` 返回、**一律作为 trap 候选**（`as_trap_kind()` 给具体
`TrapKind`，`Divergence` 为待诊断面收口的 trap 占位），不再沿用 `phi_warning` 的
warning-only 口径。

**未动**：`lib.rs` 的 `phi_warning` 运行期调用、`rules.rs`、`state.rs` 的现有
`RegState::merge`、诊断输出格式、任何 A2/A3 运行期战绩。本期是合并格本体与证据，
运行期分派接入留给 A1-e 之后的收口，不把 A2/A3 战绩冒充本期。

## 2. 完整语义口径（无简化）

- 快照四表完整（含 `lock_refs` 与 `origins`），round-trip 后可继续走
  `release_borrow` 并正确还原 origin 锁；不是只快照 mask 的简化实现。
- 合并对四表逐项处理：origins 相等保留、相异置 None；lock_refs 取 max；flags 取或。
- 良性位差异（如 `0x101` 与 `0x281`）按位或为 `0x381`，四个派生位全部保留，
  `mask_components` 列出 `Active/Fallible/Immutable/InteriorPtr` 全名，不删位。

## 3. 证据

| 项 | 结果 |
|---|---|
| fail-before（同一最终测试文件对 A1-c 快照树编译） | `rc=101`，`error[E0432]: unresolved import sa_verify::merge`，无 test result |
| pass-after（工作树） | `rc=0`，`test result: ok. 7 passed; 0 failed` |
| `cargo test --workspace` | `rc=0`，全部测试目标 0 failed |
| 968 名 `sars check`（A1-c 快照树 vs 本期） | 962 / 962，pass→fail=0，fail→pass=0 |
| 证据文件 | `DELIVERY/a1d_fail_before_pass_after.log`、`DELIVERY/a1d_harness.py` |
| 单元测试 | `crates/sa-verify/tests/a1d_merge_lattice.rs`（7 项） |

七项独立验收：快照 round-trip、相等无冲突、读/写锁冲突 trap、consumed 冲突
trap、借用分歧 trap、良性四位保留、lock_refs/origins/flags 合并。

## 4. 状态与交接

- 本期独立单元，未借 A2/A3 战绩，未用简化语义冒充完整语义。
- A1-e（诊断面：`TrapReport` 式 register/expected/actual mask 名字，对应 G1/G2）接期开工。
- 冻结面未触及：无回边、无 JS callback/float/codeval、无 81 sa_tmp、无 963/964。
