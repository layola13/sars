# B7-1 + B8 — 解释器两处分叉修复（sci parity）

- 依据：muse #19921② B7 优先级裁定——B7-1（vtable 间接调用）与 B8（bitcast i64↔ptr 往返）并列先行。
- 基线：`3d31546`（B5-fix + B6 + B7 普查均已上线）；sci 参考二进制 `/home/ctyun/sa_all/sci/zig-out/bin/sa`（头 `500477e5`）。
- 面：`sa-interp`（解释器 run 面），不触碰 verifier / 冻结项 A3/E2。

## B7-1 解释器 vtable/callback 间接调用目标解析

- fail-before（复现件 `DELIVERY/B7_SURVEY/b7_1_vtable_indirect.sa`）：`sars run` rc=1 `RuntimeTrap: invalid indirect call target 7022308338648768867`；`sci run` rc=7（正确调用 @target_fn 返回 7）。
- 根因：`exec.rs` CallIndirect 把 `load t+0 as ptr` 得到的目标值当 `program.functions` 的数值索引。vtable slot 实际存的是 UTF-8 字节 `call:@target_fn`（前 8 字节 `call:@ta` → u64 `0x6174403a6c6c6163`），不是索引。sci 通过 RegValue 的 `vtable_slot_name`/`call_target_name` provenance 解析目标名。
- 修复（最小侵入，`exec.rs`）：
  1. `Interpreter` 增字段 `indirect_targets: HashMap<u64, String>`。
  2. `new()` 构造常量时，遍历每个 `ConstValue::VTable(entries)`，按条目在常量数据中的字节偏移取前 8 字节作 key、`entry.vtable_target()` 作 value，预建 slot 值→目标函数名表。
  3. CallIndirect：目标值命中该表则按名 `self.call(name, vals)`；未命中回退原数值索引逻辑（保持既有间接调用输入兼容）。
- pass-after：`sars run` rc=7，与 sci 一致。命中语料 5 例（`463_vtable_branchy_pick`、`485_vtable_param_float`、`638_vtable_sub2_code`、`769_callback_gt_code`、`878_vtable_multi_code`）双边 run 一致。

## B8 解释器 bitcast i64↔ptr 全 64 位往返

- fail-before（复现件 `DELIVERY/B7_SURVEY/b7_2_bitcast_i64_ptr.sa`）：`m=-1 as i64; p=bitcast m as ptr; q=bitcast p as i64; ok=eq q,-1`——`sars run` rc=1（走 L_ERR，q 被当成大正数）；`sci run` rc=42（正确走 L_OK）。
- 根因：`value.rs` Bitcast 的 `Ptr→Int` 走 `_ => Ok(Value::Int(x))`，其中 `x = int(&a) = a.to_i128()` 对 `Value::Ptr(u64::MAX)` 是零扩展的 `18446744073709551615`（正数）。sci 的 `bitcastValueImpl` 是纯位保留（只改类型标签、payload bits 原样），i64 目标下应符号解释为 -1。
- 修复：`_ => Ok(Value::Int(mask(x, ty)))`，按目标宽度/符号性重新解释位模式，保证 i64↔ptr 往返位精确。Float→Ptr/Int 与 Ptr→Float 分支不变。
- pass-after：`sars run` rc=42，与 sci 一致。命中语料 `398_bitcast_i64_ptr` 双边 run 一致。

## 全门

- `cargo test --workspace`：355 passed / 0 failed。
- `968 rosetta`（`.eval/sars_968.py target/release/sars`）：rc0=962 sha `830304a687cf…e2922` / rc1=6 sha `8f89c17e76…4d55`，与基线双 sha 逐字全等、零翻转。（两处均为 run 面修复，verifier 面 rosetta 稳定符合预期。）

## 附带（#19921② 文档补正，同批）

- `B7_DIVERGENCE_CANDIDATES.md`：sci check 超时「6 例（含 36_...）」更正为实测 5 例（依 `b7_check_results.jsonl`）；补记 `965_npm_handle_ctor_method` 的 C 发射执行分叉为低优先跟进项（原稿未登记）。

## 未扩围

- B9（35 例参数寄存器出口 MemoryLeak 漏报）、lifecycle 漏报（未配对 release 停 LOCKED_MUT、sci 报 1012，单列编号与 B9 同批）、B10（报文差异）按 #19921② 排序，本批不动；A3/E2 冻结不变。
