# A1-a 状态载体 — 独立单元 fail-before / pass-after 交付

- 裁定：② 拆子项；本期只做五期中的第一期 A1-a（状态载体）。
- 工作副本 `/content/oc-box-1-work/sars`，任务分支 `task/simpl-gaps-a1`，
  基线 `69a5ff79bc39b557394b8571eee1f89ad96fe7a4`（未提交，工作树交付）。
- oracle：`/content/sa_all/sci/src/common/capability.zig:3-19/29-45/74-100`。

## 1. 本期范围（严格不越界）

落盘的是**载体本身**：`u16` 位常量、`CapabilityMask` 新类型、`maskName` 同位名字、
复合位分解、`Op`/`TrapKind`/`Transition` 与 sci 逐条一致的 20 项 `TRUTH_TABLE`，
以及只做精确匹配的 `lookup` 表访问器。

未动 `rules.rs` / `state.rs` 的现有 `RegState` 裁决路径；未接 FFI/call/try 规则；
未改 phi 合并、诊断格式、任何 A2–I5 行为。下游战绩一律不记入本期。

## 2. 完整语义口径（无简化）

- 12 位全部按 sci 落值：`0x0001..0x0400`，含 `untracked`/`fallible`/`immutable`/
  `interior_ptr`/`owns_children`；没有任何位用旁路 `HashMap` 模拟。
- `maskName` 与 sci 的 `switch` 逐字一致：单项给专名，`0x101`/`0x33` 等复合态给
  `"Composite"`，不用单个名字冒充复合态。
- `mask_components` 对复合态按位升序列全名；`0x101 → [Active, Immutable]`，
  `0x33 → [Active, Locked_Read, BorrowView, FfiBorrow]`。
  预案里 `0x33` 简写作「ffi_borrow|borrow_view」，但 `ffi_assume_borrow: 0x40→0x33`
  的 0x33 实际是四位合成；本期按 sci 全位落地，不删位、不近似。
- `TRUTH_TABLE` 20 项逐字段（prev_mask/op/legal/new_mask/trap）对拍 sci，
  含 `0x101` 上 `transfer_own` 非法（`transfer_into_immutable`）与
  `ffi_assume_borrow → 0x33`。`lookup` 只做 `prev_mask`+`op` 精确查表，
  无 if-else 近似、无回退泛化。

## 3. 证据

| 项 | 结果 |
|---|---|
| fail-before（同一最终测试文件对基线树编译） | `rc=101`，`error[E0432]: unresolved import sa_verify::mask`，无 test result |
| pass-after（工作树） | `rc=0`，`test result: ok. 5 passed; 0 failed` |
| `cargo test --workspace` | `rc=0`，全部 20 个测试目标 0 failed |
| 968 名 `sars check`（基线 vs 本期两棵 release 树） | 962 / 962，pass→fail=0，fail→pass=0 |
| 证据文件 | `DELIVERY/a1a_fail_before_pass_after.log`、`DELIVERY/a1a_harness.py` |
| 单元测试 | `crates/sa-verify/tests/a1a_state_carrier.rs`（5 项） |

fail-before 的语义依据：基线 `FnState` 是 `RegState` 裸 enum + 并行 HashMap，
既无 12 位 `u16` 字段，也无复合态名字/真值表；测试在编译期即无法引用载体 API。
本次以最终测试文件的真实编译失败取证，非事后补写。

## 4. 状态与交接

- 本期是独立单元，可单独验收；未借 A2–I5 任何战绩，未用简化语义冒充完整语义。
- A1-b（分配/借用/移动/释放四组转移，逐条真值表 fail-before/pass-after）待下一令开工。
- 冻结面未触及：无回边、无 JS callback/float/codeval、无 81 sa_tmp、无 963/964。
