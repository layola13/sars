# I1 — 原子序模型（对标 sci atomicKey / sameAddressRmwCompatible）

状态：已落地，968 check 与默认/legacy parity 零回归。基线 HEAD `65060ba`。

## 语义收敛

- **key 精度**：sci `atomicKey = (reg << 32) | offset`（`const_trap.zig:210-217`）。旧 sars 仅以 `addr.base.to_string()` 为 key，丢失 offset：同一 base 不同偏移的原子操作被误判为同址。现改为 `format!("{base}::{offset}")`，等价对齐。
- **兼容规则**：sci `sameAddressRmwCompatible(prev, next)`（`common/atomic.zig:92-96`）= 相同序，或任一为 seq_cst。旧 sars 用 `previous != order` 过严，把合法的 `relaxed → seq_cst` 误报为 mismatch。现按 sci 规则 + seen bitmask（记录该地址历史上出现过的所有序，而非仅最后一个）。
- **Cmpxchg 接入**：sci `armCmpxchg` 也调 `checkAtomicOrdering`；sars 旧版 Cmpxchg 落 default 臂无原子序检查。现新增 Cmpxchg 臂，复用 check_atomic + reads + define。
- `RuleCtx.atomics` 与 `check_function` 的 HashMap 值类型从 `AtomicOrdering` 改为 `u8`（seen bitmask），双后端共用。

## 证据

- fail-before：在基线 `rules.rs`/`lib.rs` 上跑新测试，3/4 `FAILED`（seq_cst 混合、不同 offset、cmpxchg 历史），保留语义项（relaxed→acquire mismatch）通过；`baseline_rc=101`。
- pass-after：`cargo test --test i1_atomic_ordering -p sa-verify` 4/4（双后端各跑一遍）；`cargo test --workspace` rc=0。
- 968 check：rc=0 962 / rc=1 6，sha256 `830304a6…` / `8f89c17e…` 与基线逐字一致。语料中唯一 cmpxchg（demo 108，acquire/relaxed）与唯一 atomic_rmw（demo 109，seq_cst）均正常通过。
- 默认 vs `--legacy-state`：968/968 stdout+stderr+rc 逐字节一致（diff=0）。

## 边界

- `AtomicLoad`/`AtomicStore`/`Fence` 未接入 check_atomic：sci `atomicKey` 对这两类返回 null（只 check cmpxchg/atomic_rmw），Fence 无地址，故对齐 sci 不接入。清单 I1 原文提及这三类，但对照 sci 源码后确认不应接入（sci 本身不检查）。
- 未触碰冻结面：R1 回边、JS callback/float/codeval、81 sa_tmp、963/964 校验值。
