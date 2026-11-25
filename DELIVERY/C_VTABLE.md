# C(vtable) 簇交付（接 35518dc fallible，回主会话 07:05 派单）

- 口径：一类一提交、fail-before/pass-after、pass→fail=0、禁简化；只追 `build_exe_fail_classify.json`
  中 `vtable` 标签的 112 名，不碰 extern 28 / native 1，也不碰其他簇账。
- before = `35518dc1a26e221fff708c475bf9c3b95e222609`（远端 main，工作树干净）；
  after = 任务分支 `task/vtable-c-backend-35518dc` 本提交。
- 工作副本：`/content/oc-box-1-work/sars`（一人一份；`../sci` 软链到 `/content/sa_all/sci`）。
- 只改 `crates/sa-emit-c/src/{emit.rs,tests.rs}`，另附 `DELIVERY/c_vtable_harness.py`
  与 `DELIVERY/c_vtable_fail_before_pass_after.log` 取证件。

## 根因与落地形态

`emit.rs` 原在两处显式拒收，故 112 名全部在 `build-exe` 阶段即失败：

1. `const_data()`：`ConstValue::VTable(_)` 发 `[E1024] VTableSignatureMismatch:
   vtable const … is not supported by the C backend`。
2. `emit_instruction()`：`InstructionKind::CallIndirect` 发 `[E2002]
   UnsupportedInstruction: call_indirect / vtable dispatch`。

本笔按 sci 的 C MVP 形态补 `sa-emit-c` 单侧缺口（**不是** D22 式对齐拒收）：

1. **vtable const**：每个 vtable const 落为
   `static const uint64_t sa_cN[] = {(uint64_t)(uintptr_t)&<slot target>, …};`，
   槽位按 `ConstValue::vtable_target()` → `program.functions` 求符号，缺失槽发 0；
   `static_bytes` 路径不动。
2. **声明次序**：`prototypes()` 提到 `const_data()` 之前，消除槽位初始化时的
   `use of undeclared identifier`；`const_expr()` 在指针上下文把 vtable 常量数组
   显式转成 `uint8_t *`，消除 `const uint64_t[]` → `uint8_t *` 的不兼容指针赋值。
3. **call_indirect**：发 `((<ret> (*)(<params>))(uintptr_t)(fp))(args…)`，
   即经 vtable 槽位存的函数指针真调用；实参按解析出的形参类型编组。
   签名解析走 `indirect_signature()`：`const_ptr()` 沿
   `Assign/Move/Borrow/RawCast/PtrAdd/Load` 回溯槽位基址，
   `stored_const_at()` 处理「对象 + 偏移的 store」并跨调用点把形参映射回调用方实参，
   再按 `slot = offset / 8` 取 `vtable_entry_sig()`；解析不到时按实参与 dest 的
   已知类型构造回退签名（不静默出 0）。
4. **dest 类型探测**：`FnCtx::new()` 增一轮，把 `call_indirect` dest 的类型设为
   解析出的返回类型，避免 f64 间接返回值被当作 I64 参与 `fcmp_eq`（464/456 等）。

`dump-c ../sci/demos/rosetta/560_vtable_dual_code/main.sa` 关键片段形态：

```c
static const uint64_t sa_c1_DUAL_VT[] = {
  (uint64_t)(uintptr_t)&sars_f0_ret_zero,
  (uint64_t)(uintptr_t)&sars_f1_ret_one
};
...
  r2_fp0 = *(uint8_t * *)((r1_t));
  r4_c0 = ((int64_t (*)(int64_t))(uintptr_t)(r2_fp0))((int64_t)0);
```

## fail-before / pass-after

复现件：`DELIVERY/c_vtable_harness.py`（968 rosetta 名，before/after 两个二进制各自
`build-exe` + 实跑，记录 rc 与逐字 stdout），日志
`DELIVERY/c_vtable_fail_before_pass_after.log`。

| 指标 | before(35518dc) | after(本笔) |
|---|---|---|
| 全语料 | 968 | 968 |
| rc=0（build+run 双绿） | 788 | **900** |
| vtable 112 名 rc=0 | 0 | **112** |
| pass → fail | — | **0** |
| stdout 回归（原本双绿者逐字比对） | — | **0** |

- fail-before → pass-after：**112/112**。逐名 stdout 命中 07=`77\n`、464=`464\n`、
  539=`539\n`、560=`560\n`、198=`2\n`、456=`456\n` 等（全部 112 名见日志中
  `before_rc=1 after_build_rc=0 run_rc=0` 行）。
- 簇外溢出：**0**（recovered 112 名全部在 vtable 名集内；两次 968 名跑批同参同机）。

## 验证

- `cargo test --workspace` 全绿；新增 `emits_vtable_const_and_call_indirect`，断言
  `static const uint64_t …VT[]`、槽位函数符号地址与
  `((<ret> (*)(<params>))(uintptr_t)(fp))` 调用形态，且无残留拒收诊断。
- before 二进制 sha256 = `58aac07533c8a8d9dae75625063de14091f5d41042abb0b1cef9df0b96ad5678`
  （build-exe 于 35518dc 原样编译）；after 为本提交 `target/release/sars`。
- 968 名 after 产物全部实跑，退出码与 stdout 已记入日志；pass→fail=0、stdout 回归=0。

## 后续（按主会话队列）

vtable 112 交付后依次 extern/shim **28** → native **1**；下一笔在
`task/vtable-c-backend-35518dc` 同分支续推或按主会话口径另开任务分支。
推送凭据：本机未发现 GIT_TOKEN，本地头报主会话代推，请核远端 `layola13/sars main`。
