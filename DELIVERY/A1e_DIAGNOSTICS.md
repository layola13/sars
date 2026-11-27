# A1-e 诊断面 — 独立单元 fail-before / pass-after 交付

- 裁定：② 拆子项；本期只做五期中的第五期 A1-e（诊断面收口）。
- 工作副本 `/content/oc-box-1-work/sars`，任务分支 `task/simpl-gaps-a1`，
  基线 = A1-d 工作树快照（无诊断面），工作树交付。
- oracle：`sci/src/sax/common/trap.zig:60-89`（`TrapReport` 字段）、
  `sci/src/verifier/sax_trap_report.zig`、`common/capability.zig:29-45 maskName`。

## 1. 本期范围（严格不越界）

新增 `crates/sa-verify/src/report.rs` 并由 `lib.rs:6` 导出：`TrapReport`（trap、
register、expected_mask、actual_mask、message、hint）及 `report_from_decision`。
`expected_mask` 由真值表中该 op 的第一个 legal 行反推（表驱动，不硬编码），
`expected/actual_mask_name` 走 A1-a 的 `mask_name`，复合态给 `"Composite"` 而非
任一单字段名。`to_json()` 按 sci `TrapReport` 字段名顺序输出，含 trap_code/
source_text/upstream_loc/function/is_ffi_wrapper 等 null 占位。

**未动**：`rules.rs` 的 `Diagnostic` 输出、`lib.rs` 的 `DiagSink`、运行期诊断格式、
任何 G1/G2 战绩。本期是诊断面本体与证据，运行期接入留给整体收口裁定。

## 2. 完整语义口径（无简化）

- 合法转移与表外组合都不产出报告（`None`），不伪造 trap；只有真值表判非法的
  组合才出 `TrapReport`。
- 全部非法表行的 trap 名与 sci `trapName` 逐字一致，hint 非空且按 trap 分类。
- 复合态 `0x101` 的 actual 名给 `"Composite"`，不用 `Immutable` 单字段冒充。
- JSON 字段顺序与 sci 一致，便于主会话对拍 `sax_trap_report.zig` 口径。

## 3. 证据

| 项 | 结果 |
|---|---|
| fail-before（同一最终测试文件对 A1-d 快照树编译） | `rc=101`，`error[E0432]: unresolved import sa_verify::report`，无 test result |
| pass-after（工作树） | `rc=0`，`test result: ok. 5 passed; 0 failed` |
| `cargo test --workspace` | `rc=0`，全部测试目标 0 failed |
| 968 名 `sars check`（A1-d 快照树 vs 本期） | 962 / 962，pass→fail=0，fail→pass=0 |
| 证据文件 | `DELIVERY/a1e_fail_before_pass_after.log`、`DELIVERY/a1e_harness.py` |
| 单元测试 | `crates/sa-verify/tests/a1e_diagnostics.rs`（5 项） |

五项独立验收：register/expected/actual 名字、sci JSON 字段序、复合态不简化、
合法/表外无报告、全部非法表行都有名字与 hint。

## 4. 状态与交接

- 五期 A1-a…e 全部完成，各有独立 fail-before/pass-after、workspace 全绿、
  968 名 962/962 pass→fail=0；未借 A2–I5 战绩，未用简化语义。
- 冻结面未触及：无回边、无 JS callback/float/codeval、无 81 sa_tmp、无 963/964。
- 待裁定分叉（已上报，不自磨）：`A1_FULL_SEMANTICS_PLAN.md` 的「目标形态」要求把
  `rules::transfer` 分派改为 prev_mask→查表→trap/更新，并把 `RegState`/并行 HashMap
  收敛为 mask；但 A1-b 明令「不动调用/FFI」，而 `apply_cap`/`handle_call` 与
  `state.rs` 的并行表深度耦合，完整收敛必然触碰调用/FFI 与运行期诊断口径。
  五期均以 sci 忠实组件的独立单元落地、运行期零改动；是否进入运行期接入
  （会触碰调用/FFI、可能改变部分诊断与 rc）请主会话裁定，未裁定前不动。
