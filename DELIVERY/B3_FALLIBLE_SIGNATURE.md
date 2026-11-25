# B3 交付：fallible 签名 → wasm 完整状态/值 ABI

- 解冻口径：10503/10529；按清单价值序接 R1 之后推 B3（fallible 签名 43）。「禁止简化」按质量标准执行，未降级、未停线。
- 基线：`57cb57f`（R1 已推）；本笔仅改 `crates/sa-emit-wasm/src/lib.rs`、`crates/sa-emit-wasm/src/lower.rs`，另附 `DELIVERY/` 取证件。清单 `SIMPLIFICATION_GAPS.md` 已在 57cb57f 入库。
- 复现件：`DELIVERY/B3_fail_before_pass_after.log`（逐名 before/after）、`DELIVERY/B3_harness.py`（同机同桩全语料脚本）。

## 根因

`signature_type()` 对 `return_fallible` 签名仍只发一个结果值，`lower_instruction()` 的 `Call` 臂直接诊断
`UnsupportedType: fallible call signatures are not supported` 并跳过整条调用。sci 的 fallible ABI 是
`{status, value}`（`emit_c/preamble.zig:192`，LLVM 多结果返回），`?` 检查 status 非零则原样早返回整个对象
（`emit_js/dispatch.zig:72-87`，`verifier/verify_body_arms_call_return.zig:243-246`）。sars 的 wasm 面既没有
status 结果位，也没有 status 本地槽，所有 `T!` 调用不可发射，43 名 built-fail（rc=2001）。

## 落地形态（完整语义，非桩）

1. **签名层**：fallible 签名发射两个 wasm 结果 `[i32 status, storage(value)]`（void 时仅 status）；
   `functype` 本就支持多结果数组，与 `call_indirect` 的类型索引共用同一编码。
2. **状态槽**：`Registers` 为每个寄存器配一条平行 i32 status local；函数入口统一初始化为 0。
3. **调用**：fallible 调用返回 `[status, value]`，按栈顶顺序先存值、再存 status 到 dest 的状态槽。
4. **`?`**：读源 status；非零则在 fallible 函数中压回 `(status, value)` 并 `RETURN` 原样传播（非 fallible
   函数走 trap —— sci 的 verifier 本就拒绝该形状），零值则解包出值并把 dest status 归零。
5. **返回**：fallible 函数的 `Return`/`EarlyReturn` 压 `status` 再压 `value`；返回 fallible 调用结果时
   status 取自该源的状态槽，不是恒 0；函数尾 fallthrough 也补 status。
6. **Move/Assign**：同步复制状态槽，避免 fallible 值搬移后丢失 status。
7. **`?` 目标寄存器类型**：`collect_registers` 的探测轮从源操作数推 `Try` dest 的存储类型
   （此前无 `as` 标注时一律 I64，会把 f64 负载截断成整数；425 等 8 名因此仍走错误分支）。

## fail-before / pass-after（名集对照）

方法：同一机、同一零值类型化 extern 桩（i32/i64/f32/f64 按导入签名返回 0），before=`57cb57f`、
after=本笔，968 个 `rosetta/*/main.sa` 逐个 `build-wasm` + wasmtime 实例化/执行；
trap=2002、build-fail=2001、runner-exception=2005，退出码取最后一个 wasm 结果（fallible main 取 value）。

| 指标 | before | after |
|---|---|---|
| 全语料 | 968 | 968 |
| rc=0 | 857 | 902 |
| B3 43 名 rc=0 | 0 | 43 |
| B3 43 名 build-fail(2001) | 43 | 0 |
| **pass → fail** | — | **0** |

- fail-before → pass-after：**43/43**（完整名集见日志 `RECOVERED B3` 段）：
  `50_error_chain, 176_result_flattening, 180_try_trait_v2, 366_try_unpack_const … 371_try_in_loop,
  373_try_mem_roundtrip … 375_try_branchy_returns, 416_fallible_f64_const … 435_fallible_mem_overwrite,
  512_chain3_int … 521_callback_fallible_split, 526_loop_code_check`。
- 簇外正当外溢 **2 名**：`19_result_question, 464_vtable_fallible`；
  同根（fallible 签名），其中 464 正是 10159 登记的「direct call @render()->f64! 撞 fallible 签名 → 归 B3」，
  仅因 10053 分簇表把它记在 B1 名下而未进 43 名集。不是口径稀释，单列待主会话复核。
- 其余 902−857−43−2 = 0 名无 unexplained 翻转；pass→fail 名集为空。
- 取证口径更正：首版脚本把 wasmtime 单结果函数返回的裸 `int` 误判为通过（`int[-1]` 异常被吞成 0），
  已改为按 `int`/`Val`/`list` 正确取退出码并重跑全语料；上面是更正后数字，日志与脚本均已同步。

## 验证

- `cargo test --workspace`：全绿（33 项 wasm，新增 2 项：`fallible_call_emits_status_and_value_results`
  检查 `[i32,i64]` 双结果，`fallible_call_and_try_emit_status_checks` 检查 CALL 后双槽存储 + `?` 的
  IF/RETURN 状态传播）；另修正 R1 遗留的 `backedge_lowering_emits_loop_instead_of_unreachable`
  对新增 local 操作数字节的误计（改为按 `(opcode,0x40)` 结构计数，断言不变）。
- wasmtime 对 after 的全部 902 个 rc=0 模块完成校验 + 实例化 + 执行；模块结构由 wasmtime 校验器覆盖。
- JS/interp 面本笔未动；JS vtable 20 个非 B1 翻转仍不计任何指标。

## 后续

按清单价值序续推：输出分歧 39 → R3 float/类型校验 18 → B2 atomics 4 → R2 stub 类型 3 → B4 UnknownRegister 1。
每簇仍 fail-before/pass-after + 名集对照，pass→fail 必须为 0；10 分钟一提交推送。
