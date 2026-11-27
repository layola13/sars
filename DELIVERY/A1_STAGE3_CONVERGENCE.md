# RESULT — A1 收敛（stage 3：默认 CapabilityState + 真值表主裁决）

- 基点：`origin/main` `4043ef0`（A1a–e + stage-2 regstate default-off）。
- 本轮改动后 HEAD（本地提交前）：默认启用 `CapabilityState`；`--legacy-state` 显式回退；
  FFI 三指令接入真值表；非法真值表行成为主裁决（Alloc 例外，见下）；
  泄漏检查从单载体读取 live 状态。
- 验收口径：`cargo test --workspace` 全绿；968 名 rosetta `sars check`
  rc 名集逐字不变；默认路径与 `--legacy-state` 路径 stdout+stderr **968/968 逐字节一致**。

## 1. 具体落地

| 项 | 改动 |
|---|---|
| 默认载体 | `VerifyOptions::default().use_regstate = true`（`sa-verify/src/lib.rs:37`）。CLI `check` 默认走 `CapabilityState`，`--legacy-state` 回退 `FnState`；`--regstate` 保留为兼容开关。 |
| FFI 真值表 | `runtime_mask::plan` 新增 `RawCast → FfiRawCast`、`AssumeSafe → FfiAssumeSafe`、`AssumeBorrow → FfiAssumeBorrow`，`LookupPlan.ffi_dest` 把 `new_mask` 写到目的寄存器。 |
| 主裁决 | `runtime_mask::apply` 对非法行先于旁路规则发 trap（旁路已报同类诊断时抑制重复）。Alloc 保持 confirmation-only：20 行表的 `0x08+Alloc` 是教学不变式，而 sci live `CapabilityTable.noteAlloc` 明确允许 consumed 槽位重启为 Active。 |
| 泄漏面 | `RegStateBackend::is_owned_alloc` 新增；`CapabilityState` 用 `mask & ACTIVE != 0 && CONSUMED/BORROW_VIEW 未置位` 判定 live alloc，`FnState` 仍按 `Owned{Alloc}`。 |
| 未动 | `Call/CallIndirect/Try` 的 fallible/签名状态仍走命令式规则（超出 20 行表）；`phi_warning` 仍为 warning（归 A2/G1）；FFI wrapper 上下文陷阱归 B2；`PtrAdd/Atomic*/Native` 归 E1/I1/JS 等独立清单项。 |

## 2. 对拍证据

| 检查 | 结果 |
|---|---|
| `cargo test --workspace` | rc=0，所有目标 0 failed |
| 968 名 rc 桶 | rc=0: 962，sha256 `830304a687cf6de03c1cb9650ba7e51051906a577918bb48a762e87b980e2922`；rc=1: 6，sha256 `8f89c17e76e91f9538cebcdd5bbca74b00907be77ff8f0d2baace7adeb254d55`；与 stage-2 基线逐字一致 |
| 默认 vs `--legacy-state` | stdout+stderr+rc 968/968 逐字节一致，diff 名单为空 |
| fail-before | 把新测试装回 `4043ef0` 基线树：`error[E0609]: no field ffi_dest on type LookupPlan`，编译期即失败 |
| pass-after | `stage3_call_ops_stay_on_bypass_ffi_ops_get_plans`、`stage3_illegal_rows_are_authoritative_except_alloc`、`stage3_alloc_on_consumed_defers_to_live_notealloc_semantics`、`stage3_new_path_is_default_on` 全绿 |

## 3. 边界说明

本轮完成的是 A1 的**可验证收敛边界**：能力状态载体默认化、真值表成为已接线指令的主裁决、
FFI 三指令入表、泄漏判定读单载体。剩余 `Call/Try` 状态建模、phi 冲突升级为 trap、
FFI wrapper 上下文检查分别属于 A2/B2/D1 等独立清单项，按冻结面纪律不并入本轮。
