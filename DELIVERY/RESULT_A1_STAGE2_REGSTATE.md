# RESULT — A1 stage-2（regstate_only）· CapabilityMask 键状态载体 / default-off

- 裁定：R1374（Jev 0.80 + 氢 0.58 同向）；硬条件按 R1292 原文不变。
- 工作副本：`/content/oc-box-1-work/sars`，分支 `task/simpl-gaps-a1`；开工前基线为同树未提交快照 `/tmp/opencode/stage2_evidence/before`（stage-1 + A1-a…e + J1/J20/B5/D3 均在）。
- 交付形态：工作树交付，未提交到 `origin/main`；由主会话同步验收。
- 工具链：`/tools/rust-1.75.0/bin`；`Cargo.lock` 由 v4 降为 v3（cargo 1.75 不解析 v4，非 Rust 源码改动）。

## 1. 范围（严格按令）

**做**：新增 `crates/sa-verify/src/regstate.rs`，把 `FnState` 的六张并行表（`regs` / `fallible` / `borrow_origin` / `origin_base` / `escaped` / `borrow_params`）连同 stage-1 的 `masks` 旁路表塌缩为**每寄存器一条 `CapEntry`**：`mask: Option<CapabilityMask>` 为唯一能力状态载体，`origin` / `base` / `kind` / `lock_refs` / `mutable` / `flags` 仅为必要的来源元数据（`regstate.rs:21`、`:152`）。兼容性通过 `RegStateBackend` trait（`state.rs:159`）接入：新增 `CapabilityState`（`regstate.rs:231`）与 `FnState`（`state.rs:181`）双后端，并给出双向投影（`regstate.rs:345`、`:376`）。

**不做**：`rules::transfer` 的 `match &ins.kind` bypass 分派表逐字未动；`runtime_mask::plan` 对 `Call/CallIndirect/Try/RawCast/AssumeSafe/AssumeBorrow` 仍返回 `None`；`apply_cap` / `handle_call` / `check_leaks` / `phi_warning` 未收敛；FFI 接入与分派收敛不在本阶段。所有对 `FnState` 字段的直接访问改为 trait 方法，语义逐条等价（default-off 路径仍以 `FnState` 为后端）。

**default-off 门**：`VerifyOptions.use_regstate` 默认 `false`（`lib.rs:27`、`:37`）；CLI 新增 `sars check <file> --regstate`（`cli.rs:16`、`:133`），经 `pipeline::compile_with_options`（`pipeline.rs:102`）透传（`commands.rs:81`）。门关时走 `check_function::<FnState>`，门开时走 `check_function::<CapabilityState>`（`lib.rs:216`、`:239`）。

## 2. 名集对照（before / after）

| 桶 | before 名集 | after 名集 | 逐字一致 |
|---|---|---|---|
| rc=0 | 962 名，sha256 `830304a687cf6de03c1cb9650ba7e51051906a577918bb48a762e87b980e2922` | 同左 | True |
| rc=1 | 6 名，sha256 `8f89c17e76e91f9538cebcdd5bbca74b00907be77ff8f0d2baace7adeb254d55` | 同左 | True |
| 全集 | 968 名，sha256 `25a6169a30d015f53038f4185f517d1e8ecaef453862f7e8209279569de1cc7c` | 同左 | True |

rc=1（负例，六名逐字）：`205_pkg_cyclic_dependency_reject`、`207_pkg_multiple_versions_conflict`、`226_mod_cyclic_import_detect`、`227_mod_shadowing_prevention`、`243_contract_sig_mismatch_link`、`502_bare_crossdir_import_reject`。

逐名完整名单见 `DELIVERY/stage2_fail_before_pass_after.log` 的 `before_names=` / `after_names=` 行。

源码 blast radius（`crates/**/*.rs` 逐字节）：
- added：`crates/sa-verify/src/regstate.rs`
- changed：`sa-cli/src/cli.rs`、`sa-cli/src/commands.rs`、`sa-cli/src/pipeline.rs`、`sa-verify/src/lib.rs`、`sa-verify/src/rules.rs`、`sa-verify/src/runtime_mask.rs`、`sa-verify/src/state.rs`
- removed：无

## 3. 逐 rc 归因（968 名，双 release 树）

| 项 | before | after |
|---|---|---|
| rc=0 | 962 | 962 |
| rc=1 | 6 | 6 |
| check pass | 962 | 962 |
| pass→fail | — | 0 |
| fail→pass | — | 0 |
| REGRESSIONS / RECOVERED | — | 空 / 空 |

两桶各自 `rc_bucket_identical=True`；未触发「任一阶段翻转即停」。

## 4. default-off 字节核对（flag 关 == 现头）

| 面 | 总数 | 逐字节一致 | 差异 |
|---|---|---|---|
| `sars check` stdout+stderr（含诊断文本/顺序） | 968 | 968 | 0 |
| `sars build-js` 发射 JS（`sort(lines)` 归一化，沿用 D3 口径） | 968 | 968 | 0 |
| `build-js` rc | 968 | 968 | 0 |

补充：新路全开（`--regstate`）跑同一 968 名，rc 分布同为 `[(0,962),(1,6)]`，`on_rc_diff=` 空、`on_byte_diff=` 空——塌缩载体与旁路载体在真实语料上逐字节等价。

## 5. 验收证据

| 项 | 结果 |
|---|---|
| fail-before（同一 `stage2_regstate.rs` 对开工前快照树编译） | `rc=101`，`error[E0432]: unresolved import sa_verify::regstate`，无 test result |
| pass-after（工作树） | `rc=0`，`test result: ok. 5 passed; 0 failed` |
| `cargo test --workspace` | `rc=0`，全部目标 0 failed |
| 单元测试 | `stage2_new_path_is_default_off`、`stage2_one_entry_replaces_all_parallel_maps`、`stage2_state_operations_are_identical_on_both_backends`、`stage2_merge_is_identical_on_both_backends`、`stage2_verification_is_identical_with_new_path_off_and_on` |
| 证据文件 | `DELIVERY/stage2_fail_before_pass_after.log`、`DELIVERY/stage2_harness.py`、`crates/sa-verify/tests/stage2_regstate.rs` |

七项硬条件判定：968 全过且逐 rc 归因完成；default-off 发射输出逐字节一致；bypass 分派表、call/FFI 接入均未动；workspace 与 fail-before/pass-after 均满足。

## 6. 任务 133

本期按派单优先 stage-2，133 未动。本机同步副本中未找到编号为 133 的工单/交付件或日志（仅见语料名 `133_select_macro_race` 等非任务引用）；待主会话给出 133 的原始工单内容后，再按本机同步副本核对其缺失交付件并回报缺口。
