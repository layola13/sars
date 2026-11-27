# C4 — `#version` 行类别（对标 sci LineKind.version）

状态：已落地，968 check 与默认/legacy parity 零回归。基线 HEAD `1da7a07`。

## 语义收敛

- sci `line_classifier.zig:445-455`：`#version <digits>` 识别为 `.version` 行（纯元数据，不产生指令）；空参数、非数字、数字后带多余 token 均为 `.unknown`。
- sars 旧行为：`#version` 走 HashDirective 的 `other` 分支，报 `unknown hash directive #version`。
- 本次：parser 新增 `#version` 分支，仅当行恰为 `HashDirective("#version") + Number(全数字)` 时静默接受（不产指令、不报错），否则报 `invalid #version declaration`。

## 证据

- 语料统计：968 个 rosetta demo 中 `#version` 用法 **0 处**，收紧不翻转任何 demo。
- fail-before：在基线 `parser.rs` 上跑新测试，3/4 `FAILED`（合法形式被拒），非法形式测试通过；`baseline_rc=101`。
- pass-after：`cargo test --test c4_version_line -p sa-parser` 4/4；`cargo test --workspace` rc=0。
- 968 check：rc=0 962 / rc=1 6，sha256 `830304a6…` / `8f89c17e…` 与基线逐字一致。
- 默认 vs `--legacy-state`：968/968 逐字节一致（diff=0）。

## 边界

- sars 无 `LineKind` 枚举，本次仅在 parser 的 HashDirective 分派里等价实现接受/拒绝，不影响其他行类别。
- 未触碰冻结面：R1 回边、JS callback/float/codeval、81 sa_tmp、963/964 校验值。
