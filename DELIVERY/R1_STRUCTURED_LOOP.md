# R1 交付：wasm 回边 → 结构化 LOOP（A 案）

- 解冻口径确认：10183/10186/10187/10195 冻结令作废；「禁止简化」只作质量标准，非停工令。R1 按 A 案（结构化 LOOP 插入）推进，未降级、未停线。
- 基线：`1a9a3c8`（HEAD，B1 + JS vtable 之上）；实现仅改 `crates/sa-emit-wasm/src/lower.rs`。
- 工作图：本笔随附 `DELIVERY/SIMPLIFICATION_GAPS.md`（原样入库，未改其取证内容）。

## A 案开工点与实现

开工点：`lower.rs` 的 `branch_depth()` 对回边（`j <= current`）返回 `None`，`emit_branch()` 随即发 `DROP; UNREACHABLE`。原发射结构是「函数级单个 LOOP + n-1 个 BLOCK 的单一反向嵌套」，只能承载回块 0 的回边；任意非零头的回边在语义上无法向内分支。

落地形态：

1. `build_control_flow()` 扫描每个块的 `Jmp/Br/BrNull`，按目标块序号 `<` 当前块序号识别回边，得到每个回路头 `h` 的区间 `[h, max(回边源)]`（按头序号单调化以支持嵌套回路）。
2. `build_items()`/`plan_region()` 把函数体切成「普通块」与「回路区域」的嵌套树；每个区域（含根区域）在首块前插入独立 `LOOP`，区域内按反向序插入 `BLOCK`。
3. 分支深度改为源点相关：回边 → 该回路头的 `LOOP`；前向边 → 目标块前收尾的 `BLOCK`；`exit` → 函数外层 `BLOCK`。`ControlFlow::depth()` 用每块预先记录的构造栈求深度，构造栈与 `opens/closes` 事件一一对应，模块结构平衡。
4. 回边不再发 `UNREACHABLE`；`Panic`/带值 `Return` 的 `UNREACHABLE` 保持原义。

## fail-before / pass-after（名集对照）

方法：同一台机器、同一组 extern 桩（extern 调用统一返回 0），分别用 `1a9a3c8`（before）与本笔（after）的 `sars build-wasm` 生成模块，用 wasmtime 实例化并调用导出函数；trap 记为 2002，build 失败记 2001，运行器异常记 2005。逐名原始数据见 `DELIVERY/R1_fail_before_pass_after.log`。

| 指标 | before | after |
|---|---|---|
| 全语料 main.sa | 968 | 968 |
| rc=0 | 818 | 876 |
| rc=2002（trap） | 59 | 1 |
| rc=2001（build 失败，原子/SIMD 等，两侧同集） | 71 | 71 |
| rc=2005（extern 桩异常，两侧同集） | 20 | 20 |
| **pass → fail** | — | **0** |

- fail-before(2002) → pass-after(0)：**58 名**（完整名集见日志 `RECOVERED` 段）。
- fail-before → 仍失败：1 名 `539_callback_codeval`；该名无回边、before/after 均为 2002，系 extern 桩把 vtable 指针 load 成 0 导致 call_indirect 陷桩，属取证环境伪影，非 R1 未修，亦非回归。
- 原清单登记的「57 名」与本取证集 59 的差异来自 extern 桩与真实运行器口径差；主会话可用其运行器复核名集，本笔不做口径稀释。
- JS vtable 的 20 个非 B1 翻转仍未计入任何指标，等主会话归因复核。

## 验证

- `cargo test --workspace`：全绿；`sa-emit-wasm` 32 项（新增 3 项：前向深度、非零头回边深度、回边发射不再产生 `DROP; UNREACHABLE` 且构造平衡）。
- wasmtime 对 after 的全部 876 个 rc=0 模块完成校验 + 实例化 + 执行；模块结构（BLOCK/LOOP/END 与 br 深度）由 wasmtime 校验器覆盖。
- `cargo fmt` 仅作用于本笔改动文件；未引入跨文件格式噪音。

## 后续

按清单价值序续推：B3 fallible 签名 43 → 输出分歧 39 → R3 float/类型校验 18 → B2 atomics 4 → R2 stub 类型 3 → B4 UnknownRegister 1。每簇 fail-before/pass-after + 名集对照，pass→fail 必须为 0；10 分钟一提交推送。
