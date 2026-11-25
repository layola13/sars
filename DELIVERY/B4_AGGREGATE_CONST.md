# B4 UnknownRegister 专项交付（回 10604）

- 裁定：10503/10604 价值序 B4 UnknownRegister 1。不开桩、不降级。
- 基线：before = `37a17c5`（B2 已本地落）；after = 本笔。只改
  `crates/sa-ir/src/types.rs`（另附 `DELIVERY/` 取证件与 `crates/sa-emit-wasm/src/lower.rs`
  上一笔已经在树的原子实现，本笔未再改发射器）。
- 复现件：`DELIVERY/b4_harness.py`（同 `atomics_harness.py` 的 969 名口径）、
  `DELIVERY/b4_fail_before_pass_after.log`。

## 根因

`29_const_data` 的 `@const VALUES = struct { a: 4 = hex:…, b: 4 = …, c: 4 = … }`
被 parser 变成 `ConstValue::Aggregate(vec![Bytes;B…])`。wasm `const_layout()` 只认
`VTable` 与 `ConstValue::bytes()` 非空值，而 `bytes()` 原来仅覆盖 `Bytes/Utf8`，聚合常量
被 `continue` 丢掉——`&VALUES` 因此既不在寄存器表也不在常量地址表，发射时落入
`UnknownRegister: register VALUES is not defined`，build 失败、无 wasm 产物。这正是
清单 W14「数值/聚合常量在 wasm 产物中无声消失」的未知寄存器表现。

## 落地形态（完整语义）

`ConstValue::bytes()` 扩展为按声明顺序扁平化聚合常量：
- `Bytes/Utf8` 原样；`Bool` 1 字节；`Int` 小端 16 字节；`Float` 小端 8 字节；
- `Aggregate` 递归拼接各字段字节（嵌套聚合同样展开）。
`const_layout()` 无需改动即可把聚合常量按 8 字节对齐写进 data 段，`&VALUES` 解析到该地址，
三条 `load values+0/+4/+8 as i32` 读回 1/2/3。无桩、无别名特判、无针对 29 的硬编码。

## fail-before / pass-after

| 指标 | before | after |
|---|---|---|
| 全语料 | 969 | 969 |
| rc=0 | 918 | 919 |
| B4 目标名 rc=0 | 0 | 1 |
| pass → fail | — | 0 |

- fail-before → pass-after：**1/1**（29_const_data），带打印桩复核 stdout = `6\n`。
- `cargo test --workspace` 全绿（新增 sa-ir 聚合常量扁平化单测）。

## 后续

R2 stub 3（186/963-964）待处理：当前失败表现为 runner 桩形态问题，待复核后按口径开工。
推送继续报主会话代推。
