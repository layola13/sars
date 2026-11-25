# F 簇交付：wasm call_indirect 异构签名（回 10860 · 539_callback_codeval）

- 排序：D→E→C(wasm)→C(JS)→F→**B**；本笔只做 F[539]，不并簇。
- 裁定依据：10695 簇 F 根因段 + 10700 ③（一簇一提交、pass→fail=0）。before HEAD = `987a066`，
  before 二进制快照 `/tmp/opencode/f_before/sars`。

## 根因

`crates/sa-emit-wasm/src/lower.rs::collect_registers()` 的 `CallIndirect` 结果推断把所有间接调用
的结果类型定为「首个同参 vtable 签名的返回值」。539 的两个 vtable 条目同参不同返：

- `CB_C_VT.invoke = @cb_code` → `(i64) -> i64`
- `CB_V_VT.invoke = @cb_val` → `(i64) -> f64`

两处 `call_indirect` 都被猜成 `(i64)->i64`，`v` 的本地槽也定成 i64；而表中 `@cb_val` 的真实
wasm 签名是 `(i64)->f64`，运行期 `call_indirect` 签名校验失败 → trap 2002
（`indirect call type mismatch`）。JS 后端按 `__sa_ftable` 动态分派，不受此影响。

## 落地形态（真修，非简化）

新增三个纯函数（`lower.rs:265-360`）：

- `vtable_entry_sig(entries, index, program)`：按 vtable 槽位取目标函数签名。
- `vtable_sig_from_const_addr(base, offset, ...)`：沿 `Borrow`/`Assign`/`Move`/`RawCast`/
  `AssumeSafe` 链把 `load <vt>+offset as ptr` 的基址追到 `@const ... = vtable {...}`，
  按 `offset/8` 取槽位签名。
- `vtable_sig_from_target(target, ...)`：追 `call_indirect` 的目标操作数（寄存器定义链、
  `PtrAdd`、vtable token `SA_FN_BASE+index`、`@symbol` 直接目标）得到被调函数签名。

`CallIndirect` 结果推断改为：**先按解析出的目标签名**（校验参数 storage 一致），
解析不出才回退原「首个同参签名」启发式。同时用该签名更新 dest 的 `types`/`ptypes`
与 `probe`，使后续 `fcmp_eq v, expect` 按 f64 发射；`lower_function` 的
`call_indirect` 发射路径（表索引、token→表下标换算）逐字未动，未改表结构与数据段。

新增单测 `lower::tests::indirect_call_result_uses_resolved_vtable_signature`：
两张 vtable、两个不同返回类型函数，断言两个 dest 分别得到 VALTYPE_I64 / VALTYPE_F64。
**没有任何 demo 输出硬编码**；`539\n` 来自 `main` 的 `L_OK` 分支真执行。

## fail-before / pass-after（969 wasm 全语料，stdout 口径）

同一 typed-stub runner（`mode="c"`，与 C(wasm) 账同口径），唯一变量是编译器二进制
before→after。取证件 `DELIVERY/f_harness.py`、`DELIVERY/f_fail_before_pass_after.log`。

| 指标 | before | after |
|---|---|---|
| 全语料 | 969 | 969 |
| rc=0 | 926 | **927** |
| fail → pass | — | **1**（539_callback_codeval） |
| pass → fail | — | **0** |
| 原通过名 stdout 变化 | — | **0** |

- 目标名逐名：`539_callback_codeval` before `2005`/无输出（trap）→ after rc=0 / `539\n`，
  与 sci oracle 逐字一致。
- `cargo test --workspace` **211/211** 全绿（原 210 + 新增 1）。

## 证据

- `crates/sa-emit-wasm/src/lower.rs:179-223`、`265-360`、`2519-2578`
- `DELIVERY/f_harness.py`、`DELIVERY/f_fail_before_pass_after.log`

## 后续

按序 B[#npm_bind 诊断对齐 10 名]：补 build-wasm 的 #npm_bind 门，按 10700 ①「诊断对齐」
判据验收（不要求 rc 翻绿）。build-exe 扩面另簇。本笔本地提交后请主会话代推并 PAT 核验，不停线。
