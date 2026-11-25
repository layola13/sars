# C(fallible) 原子簇交付（回 11203 · fallible 修复排下一类）

- 口径：一类一提交、fail-before/pass-after、pass→fail=0、禁简化；不追 19 的
  MemoryLeak、不追 372 的 br_null（两者各有独立账，本笔未触碰相关逻辑）。
- before = `244f5ed`（atomics 已落地、远端同步树，工作树干净）；after = 本提交。
- 只改 `crates/sa-emit-c/src/{emit.rs,runtime.rs,tests.rs}`，另附 `DELIVERY/` 取证件。
- 裁定依据：`DELIVERY/SCI_C_FALLIBLE_FACT.md` —— sci 本体支持 C 目标，故该簇为
  sars 单侧 `sa-emit-c` 缺口，照 sci 形态移植，**不是**「按 D22 式对齐拒收」。

## 根因与落地形态（照 sci 形态）

`emit.rs` 原在三处 `[E2002]` 显式拒：fallible call（`:589`）、`Try`（`?` 解包，`:651`）、
fallible 函数签名（`:683`）。本笔移除这三处拒收，按 sci C MVP 后端形态落地
（错误通道擦除、返回值 identity 包装、`?` 退化为值拷贝）：

1. **fallible 签名**：与同值类型的普通函数同形（单 C 返回值），不再诊断拒收；
   `return_fallible` 仅参与返回包装与寄存器类型推断。
2. **fallible 返回**：`Return`/`EarlyReturn` 发 `return sa_ok_val((T)(v));`，无值发
   `sa_ok_none()`；`runtime.rs` 定义 `#define sa_ok_val(v) (v)` /
   `#define sa_ok_none() 0`，对应 sci `sa_ok_val(sa_u64)` 的 identity 语义
   （据点 `sci/src/emit_c/operands.zig:489-499`）。
3. **`?` 解包**：`Try` 发 `{ T sa_t = (src); dest = sa_t; }` —— 值拷贝，错误通道擦除，
   与 sci `src/emit_c/linear.zig:127-135` 同形。
4. **fallible call**：普通调用并赋给 dest；`FnCtx` 探测轮与推断轮均从源操作数推
   `Try` dest 的存储类型（否则 f64 负载被截成整数，416/421/425 等 8 名假翻绿），
   对应 wasm 侧 B3 的同名缺口修复。

`dump-c ../sci/demos/rosetta/180_try_trait_v2/main.sa` 逐字片段：

```c
#define sa_ok_val(v) (v)
#define sa_ok_none() 0
...
int32_t sars_f0_step_left() {
L_L_ENTRY:
  return sa_ok_val((int32_t)((int32_t)2));
...
int32_t sars_f2_combine() {
  r0_left_res = sars_f0_step_left();
  { int32_t sa_t = (r0_left_res); r1_left = sa_t; }
  ...
  return sa_ok_val((int32_t)(r4_total));
```

## fail-before / pass-after

复现件：`DELIVERY/c_fallible_harness.py`（968 rosetta 名，before/after 两个二进制各自
build-exe + 实跑，记录 rc 与逐字 stdout）、`DELIVERY/c_fallible_fail_before_pass_after.log`。

| 指标 | before(244f5ed) | after(本笔) |
|---|---|---|
| 全语料 | 968 | 968 |
| rc=0（build+run 双绿） | 745 | **788** |
| fallible 43 名 rc=0 | 0 | **43** |
| pass → fail | — | **0** |
| stdout 回归（原本双绿者逐字比对） | — | **0** |

- fail-before → pass-after：**43/43**。逐名 stdout 命中 180=`7\n`、50=`12\n`、
  176=`2\n`、366=`366\n`、374=`374\n`、416=`416\n`、421=`421\n`、425=`425\n`、
  513=`513\n`、526=`526\n` 等（全部 43 名见日志 `before_rc=1 after_build_rc=0 run_rc=0` 行）。
- **未触碰 19 / 372 的独立问题**：二者本就在 43 名集内，之前的拒收原因同为 fallible；
  本笔移除该拒收后随簇翻绿（19 stdout=`21\n`、372 stdout=`372\n`）。
  未改 MemoryLeak 检查、未改 `br_null` 下沉；sci 侧 19 的 `[MemoryLeak]` 与 372 的
  LLVM `br_null` 仍是各自独立缺口，不属本簇、不顺手追。
- 簇外溢出：**0**（recovered 43 名全部在 fallible 名集内）。

## 验证

- `cargo test --workspace` 全绿；新增 `emits_fallible_call_try_and_return`，断言
  `sa_ok_val` 包装、`sa_t` 值拷贝、f64 dest 按 `double r1_v` 声明，且无残留拒收诊断。
- 968 名 after 产物全部实跑，退出码与 stdout 已记入日志；pass→fail=0、stdout 回归=0。

## 后续（按 11203 顺序）

fallible 之后依次 vtable/call_indirect **112** → extern/shim **28** → native **1**。
推送：本机无 GIT_TOKEN，本地头报主会话代推；请核远端 `layola13/sars main`。
