# C(atomics) 原子操作专项交付（回 11172 / R1192）

- 口径：按 R1192 拆分，**atomics 3 名先行**、一类一提交、pass→fail=0、禁简化。
- before = `78b45e0`（中期账头，远端同步树，工作树干净）；after = 本提交。
- 只改 `crates/sa-emit-c/src/{emit.rs,tests.rs}`，另附 `DELIVERY/` 取证件。

## 根因与落地形态

`emit.rs` 原把 `AtomicLoad/AtomicStore/Cmpxchg/AtomicRmw/Fence` 统一打成
`[E2002] UnsupportedInstruction: atomics / fences is not implemented by the C backend`，
build-exe 直接失败、无产物。落地真实 C11 原子语义（非无操作桩、非普通 load/store 冒充）：

1. `AtomicLoad/AtomicStore`：按 SA `PrimType` 选指针类型，发 `__atomic_load_n` /
   `__atomic_store_n`；排序映射 relaxed/acquire/release/acq_rel/seq_cst →
   `__ATOMIC_*`，对 load/store 不合法的排序收紧到 `__ATOMIC_SEQ_CST`（强于标注要求）。
2. `Cmpxchg`：发 `__atomic_compare_exchange_n`（strong、success/failure 双排序）；
   `dest` 收实际旧值（builtin 写回的 `sa_expected`），`extra_dests[0]` 收成功布尔，
   与 wasm 后端「old + ok」双 dest 语义一致。
3. `AtomicRmw`：`add/sub/and/or/xor/xchg` → `__atomic_fetch_*` / `__atomic_exchange_n`；
   `umin/umax/smin/smax` 用 `__atomic_load_n` + `__atomic_compare_exchange_n`
   CAS 循环实现（按有无符号选比较符），完整语义、失败重试。
4. `Fence`：发 `__atomic_thread_fence(order)`。

## fail-before / pass-after

复现件：`DELIVERY/c_atomics_harness.py`（968 rosetta 名，逐名 build-exe + 实跑，
记录 build rc / run rc / stdout）、`DELIVERY/c_atomics_fail_before_pass_after.log`。

| 指标 | before | after |
|---|---|---|
| 全语料 | 968 | 968 |
| rc=0（build+run 双绿） | 742 | 745 |
| 3 名 atomics rc=0 | 0 | 3 |
| pass → fail | — | **0** |
| stdout 回归 | — | **0** |

- fail-before → pass-after：**3/3**（108_atomic_spin_lock、109_atomic_fetch_add、
  363_fence_noop），stdout 逐字命中 108=`1\n`、109=`13\n`、363=`363\n`（与 B2 wasm
  专项交付同值）。
- `cargo test --workspace` 全绿；新增 `emits_atomics_and_fences`（断言五类原子指令
  真发 C11 builtin 且排序正确），无旧「原子必须被拒」测试需改。

## 后续（按 11172 顺序）

atomics 之后依次 vtable/call_indirect **112** → extern/shim **28** → native **1**。
fallible **43** 维持挂起，仅交付 `SCI_C_FALLIBLE_FACT.md` 一条事实；不得默认对齐、
不得静默跳过。
