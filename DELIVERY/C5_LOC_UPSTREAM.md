# C5 — `#loc` 上游位置传播（对标 sci flattener pending_loc）

状态：已落地，968 check 与默认/legacy parity 零回归。基线 HEAD `3821a28`。

## 语义收敛

- sci flattener 用 `pending_loc`（`emitparsedline.zig:47,79,84,563`）：`#loc "file":line:col` 设置上游位置，由下一条发射的指令消费一次（one-shot），用于跨文件 import/宏展开的诊断定位。
- sars 旧行为：`#loc` 仅 push 到 `program.locs`，且 HashDirective 分支无条件 `leave_function()`（导致函数体内的 `#loc` 直接结束函数），指令 SourceLoc 始终是 flat 文件行。
- 本次改动（`parser.rs`）：
  - `#loc` 不再 `leave_function`（它是行内上游标记，可出现在函数体内）；
  - Parser 加 `pending_loc: Option<SourceLoc>`，`#loc` 时由 LocDecl 的 file/line/column 设置；
  - `parse_body_line` 中若 pending 为 Some，覆盖新指令的 `ins.loc` 并清空（one-shot）；block/函数标签 loc 保留 flat 源文件位置（与 sci 一致）。

## 证据

- 语料统计：968 个 rosetta `main.sa` 中 `#loc` 行数为 **0**（`#loc` 是 flattener 内部标记，不出现在源 .sa），故收紧不翻转任何 demo。
- fail-before：在基线 `parser.rs` 上跑新测试，2/2 `FAILED`（基线 leave_function 使函数体提前结束 + 不 retarget）；`baseline_rc=101`。
- pass-after：`cargo test --test c5_loc_upstream -p sa-parser` 2/2；`cargo test --workspace` rc=0。
- 968 check：rc=0 962 / rc=1 6，sha256 `830304a6…` / `8f89c17e…` 与基线逐字一致。
- 默认 vs `--legacy-state`：968/968 stdout+stderr+rc 逐字节一致（diff=0）。

## 边界

- sars 的 `sa-flatten` 展开 import 时仍不生成 `#loc`（清单 B8/B11 的源归因缺口）；本项只让**已存在**的 `#loc` 生效，不改变 import 展开的源标记生成。
- `LocDecl.original_line` 仍等于 line（无原始行映射），不影响本次 upstream loc。
- 未触碰冻结面：R1 回边、JS callback/float/codeval、81 sa_tmp、963/964 校验值。
