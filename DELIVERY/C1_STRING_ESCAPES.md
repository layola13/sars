# C1 — 字符串转义与 UTF-8 校验（对标 sci decodeQuotedBytes / utf8ValidateSlice）

状态：已落地，968 名零回归。基线 HEAD `4043ef0`，交付提交见本仓 log。

## 语义

- 严格入口 `sa_ir::decode_quoted` / `unescape_bytes`：
  - `\\ \" \n \r \t \0` → 对应字节；`\xHH` → 单字节（两位十六进制）。
  - 未知转义、残缺 `\x`、尾部裸 `\`、非引号包裹 → Err。
  - 逐字节对照 sci `src/interp/memory.zig:285-336`，含 `parseHexDigitPair` 行为。
- `sa_ir::utf8_valid`：对标 sci `std.unicode.utf8ValidateSlice`，拒绝续字节缺失、孤立 0xFF、UTF-8 代理区。
- 宽松主路径 `Operand::parse`：新增 `\xHH` 识别（此前字面保留），其余保持历史行为；因此 968 名语料 stdout/stderr 逐字节不变。
- const 解析点：`utf8:"..."` 与 `"..."` 常量先经严格校验，非法即返回 `invalid constant value` 诊断，不再静默生成 Utf8 const。

## 证据

- fail-before：把新增测试装回 `4043ef0` 基线树，`cargo test -p sa-ir -p sa-parser` 报 `error[E0432]: unresolved imports sa_ir::decode_quoted, sa_ir::unescape_bytes, sa_ir::utf8_valid`，`baseline_rc=101`。
- pass-after：`cargo test --workspace` rc=0，32 个 test target 全 `ok`，其中 11 个 C1 用例逐条通过。
- 968 名 rosetta `sars check`：rc=0 962 / rc=1 6，sha256 `830304a6…` 与 `8f89c17e…` 与基线逐字一致（见 `.eval/c1_968.txt`，提交时该临时目录不入库）。
- 语料转义分布核查：唯一 `\x` 位于 `29_const_data/main.sa` 的 `hex:` 常量，走 `const_value` 的 hex 分支，不经 `unescape`；故收紧未知转义不翻转任何 demo。

## 边界

- 清单 C1 原文另含 lexer 层 `lex_string`（sa-flatten/src/lexer.rs:263-307）只跳 `\`+下一字符；本次未改：lexer 只负责 token 切分（保留原文），语义解码统一在 sa-ir 严格入口与 const 解析点完成，行为已等价对齐 sci，改 lexer 反而会破坏 token 原文诊断。
- `bytes:"..."` 路径仍按字节原样处理，不做 UTF-8 校验（sci 亦仅对 utf8 字面量校验）。
- 未触碰冻结面：R1 回边、JS callback/float/codeval、81 sa_tmp、963/964 校验值。
