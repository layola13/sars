# C2 — cap 前缀只剥一个（对标 sci stripTextOperandPrefix）

状态：已落地，968 check 与默认/legacy parity 零回归。基线 HEAD `bd7302f`。

## 语义收敛

- sci `stripTextOperandPrefix`（`interp/memory.zig:346-353`）只剥 `&/^/*/@` 中的**一个**前缀并 trim，剩余字符串交后续解析；叠加前缀不会被静默折叠。
- sars 旧 `Operand::parse` 用 `while` 循环剥任意数量 `&/^/*`，`&&a` 被折叠为 `Reg("a")`——掩盖了非法写法。
- 本次改为单次剥离（`if let`），`&&a` 现解析为 `Reg("&a")`，由后续 verifier 的 UnknownRegister 正常拒绝；单前缀与 `&@foo`（剥 `&` 后走 Symbol）行为不变。

## 证据

- 语料统计：968 个 rosetta demo 中多前缀 `[&^]{2,}` 用法 **0 处**，收紧不翻转任何 demo。
- fail-before：在基线 `instr.rs` 上跑新测试，`c2_stacked_prefixes_keep_the_remainder` `FAILED`（旧循环折叠）；另两项通过；`baseline_rc=101`。
- pass-after：`cargo test --test c2_cap_prefix -p sa-ir` 3/3；`cargo test --workspace` rc=0。
- 968 check：rc=0 962 / rc=1 6，sha256 `830304a6…` / `8f89c17e…` 与基线逐字一致。
- 默认 vs `--legacy-state`：968/968 逐字节一致（diff=0）。

## 边界

- `@` 前缀在 sars 仍走 `Operand::Symbol` 分支（sci 在 stripTextOperandPrefix 里也把 `@` 当可剥前缀，但其来源是 raw 字符串解析路径）；sars 的 `@foo` → Symbol 语义保持不变。
- CallArg 的 cap 前缀由 parser 层按 token（`Amp/Caret/Star`）解析，不经 `Operand::parse`，不受本次改动影响。
- 未触碰冻结面：R1 回边、JS callback/float/codeval、81 sa_tmp、963/964 校验值。
