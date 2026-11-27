# B3 — @const 数据不可变性（对标 sci seedConstSymbols / constTrap）

状态：已落地，968 check 与默认/legacy parity 零回归。基线 HEAD `90b791c`。

## 语义收敛

- sci 用 `active|immutable` 位 seed const 符号（`const_trap.zig:44-55`），并在 writeCheck/consume/release/return 等处发 `const_mutation`。sars 无 immutable 位载体，改以 `program.consts` 命名集合判定，避免全量 seed 改变 state 引发连锁回归。
- 新增 `is_named_const` / `const_mutation` helper（`rules.rs`），对命名 `@const` 符号（区别于数值字面量）：
  - `consume`（Move / call-by-move arg）→ `Trap::ConstMutation`「cannot be moved」；
  - `release` → `ConstMutation`「cannot be released」；
  - `Return` by-value → `ConstMutation`「cannot be returned by move」。
- 保留合法路径：字面量常量、`read_operand` 读取 const、`Raw`/`ByValue`/shared borrow 传参仍按原样跳过（sci 允许 const copy/共享借用）。
- 双后端一致：guard 在 state 无关的 helper 层，早于后端分派。

## 证据

- 语料统计：965 个含 `@const` 的 demo、158 个不同 const 名；const 名被 `^move` / `release` / `return` **0 处**——收紧这些 guard 不翻转任何 demo。唯一的 `\x` 常量在 hex: 分支，与本次无关。
- fail-before：在基线 `rules.rs` 上跑新测试，3 个 mutation guard `FAILED`（`baseline_rc=101`），合法读取测试通过。
- pass-after：`cargo test --test b3_const_immutability -p sa-verify` 4/4；`cargo test --workspace` rc=0。
- 968 check：rc=0 962 / rc=1 6，sha256 `830304a6…` / `8f89c17e…` 与基线逐字一致。
- 默认 vs `--legacy-state`：968/968 stdout+stderr+rc 逐字节一致（diff=0）。

## 边界

- 未做全量 immutable 位 seed 与 per-slot flags（sci `regFlagImmutable` / `isConstData`），因此「对 const 取可变借用」仍沿用既有 `CapabilityMismatch` guard（清单 B3 原文称 mutable borrow 已被拒）；writeCheck 的通用 immutability 传播（经 borrow 链）未并入本轮，属更大改动。
- 未触碰冻结面：R1 回边、JS callback/float/codeval、81 sa_tmp、963/964 校验值。
