# B2 wasm 原子操作专项交付（回 10604）

- 裁定：10503/10604 价值序 B2 atomics 4。不开桩、不降级：原子语义按 WebAssembly
  threads proposal 的真实指令发射（single-threaded 宿主上语义等价，非「无操作」冒充）。
- 基线：before = `196475e`（远端已核验头）；after = 本笔。只改
  `crates/sa-emit-wasm/src/{opcode.rs,lower.rs}`，另附 `DELIVERY/` 取证件。
- 复现件：`DELIVERY/atomics_harness.py`（968 rosetta + support/once_probe 共 969 名，
  逐名 build-wasm + wasmtime 校验/实例化/执行；单结果 extern 桩返回标量，避免把
  list 当单值传入造成的 runner-exception）、`DELIVERY/atomics_fail_before_pass_after.log`。
- stdout 复核：另用带 `sa_print_bytes` 打印桩的 runner 实跑 4 名，输出逐字命中
  108=`1\n`、109=`13\n`、363=`363\n`、once_probe=`once ok\n`。

## 根因与落地形态

原 `lower.rs` 把 `AtomicLoad/AtomicStore/Cmpxchg/AtomicRmw/Fence` 全打成
`UnsupportedInstruction: atomic operations require target atomics support`，build 直接失败、
无 wasm 产物。落地完整语义：

1. `AtomicLoad/AtomicStore`：按 SA `PrimType` 选 `i32.atomic.load(8_u/16_u)/store` 或
   `i64.atomic.load/store`，memarg 对齐取自然对齐；窄有符号 load 先做 i32 符号扩展再升 i64。
2. `Cmpxchg`：`i32/i64.atomic.rmw.cmpxchg` 真发指令；第一 dest 收 old，第二 dest（ok）由
   `old == expected` 算出；两个 dest 的 fallible 状态槽归零。
3. `AtomicRmw`：`add/sub/and/or/xor/xchg` 映射到对应 wasm 原子 RMW 指令；wasm 没有原子
   min/max，故 `umin/umax/smin/smax` 用 `atomic.load` + `select` + `atomic.rmw.cmpxchg`
   的 CAS 循环实现（完整语义，不是普通 load/store 冒充），失败重试、成功退出。
4. `Fence`：发 `atomic.fence`（前缀 `0xfe 0x03 0x00`）。wasm 内存模型下为顺序一致，
   等价于 acquire/release/seq_cst 要求。
5. 排序标记不产生额外 wasm 立即数（wasm 原子指令本身为 seq_cst，强于标注的最低要求）。

## fail-before / pass-after

| 指标 | before | after |
|---|---|---|
| 全语料 | 969 | 969 |
| rc=0 | 914 | 918 |
| 4 名 B2 rc=0 | 0 | 4 |
| pass → fail | — | 0 |

- fail-before → pass-after：**4/4**（108_atomic_spin_lock, 109_atomic_fetch_add,
  363_fence_noop, support/once_probe）。
- `cargo test --workspace` 全绿（wasm 40 项，新增 4 项：原子 load/store 前缀、cmpxchg
  双 dest、rmw add 原生 + min/max CAS 循环、fence；另把旧的「原子必须被拒」测试改为
  「原子正确发射」）。
- 918 个 rc=0 模块经 wasmtime 校验/实例化/执行；JS/interp 面未动。

## 后续

按 10604 续推 R2 stub 3（186/963-964）→ B4 UnknownRegister 1（29_const_data）。
每簇仍 fail-before/pass-after + 名集对照，pass→fail 必须为 0；推送报主会话代推。
