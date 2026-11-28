# D2 — EarlyReturn 泄漏检查（对标 sci early_return_leak）

状态：已落地，968 check 与默认/legacy parity 零回归。基线 HEAD `7a409fe`。

## 语义收敛

- sci `verify_body.zig:511-513` 把 `early_return` 和 `try_` 都路由到 `armTry`，后者扫描 src 之外的活跃寄存器，报 `.early_return_leak`（code 1026，「early return would leak live registers」）。
- sars 旧行为：`EarlyReturn` 落 default 臂，只做 read + define，不检查泄漏；`Trap::EarlyReturnLeak`（code 1026）已定义但 verifier 从不发。
- 本次：
  - `check_leaks` 拆出 `check_leaks_with(ctx, state, loc, trap)`，用调用方指定 trap，普通 return 仍用 `MemoryLeak`；
  - 新增 `EarlyReturn` 臂：read value + `check_leaks_with(..., Trap::EarlyReturnLeak)`，消息/提示为「early return would leak allocation 'p'」/「release 'p' before the early return」。
- 两后端共用（check_leaks 是 state 无关扫描）。

## 证据

- 语料统计：968 个 rosetta demo 中 `early_return` 用法 **0 处**，收紧不翻转任何 demo。
- fail-before：在基线 `rules.rs` 上跑新测试，`d2_early_return_with_live_allocation_is_leak` `FAILED`（基线不报），另两项通过；`baseline_rc=101`。
- pass-after：`cargo test --test d2_early_return_leak -p sa-verify` 3/3（双后端）；`cargo test --workspace` rc=0。
- 968 check：rc=0 962 / rc=1 6，sha256 `830304a6…` / `8f89c17e…` 与基线逐字一致。
- 默认 vs `--legacy-state`：968/968 stdout+stderr+rc 逐字节一致（diff=0）。

## 边界

- `EarlyReturn` 的 fallible 契约（src 必须 fallible）与 `?` 的 early-return 路径扫描是 sci armTry 的完整语义；本轮只落地**活跃所有权泄漏**部分，不改 Try 臂的 fallible 判定（D1 已完成）。
- 未触碰冻结面：R1 回边、JS callback/float/codeval、81 sa_tmp、963/964 校验值。
