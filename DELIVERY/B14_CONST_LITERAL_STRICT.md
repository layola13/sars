# B14 — 常量字面量严格校验（sci parity）

- 基线：`424c42b`（E4 第一批）；落地提交见下方提交号。
- 对照 sci：`common/const_decl.zig:311-330`（hex 严格 `\xNN`）、`:362-377,401-417`（struct 字段实际长度须等于声明 size）；非法字面量统一为 `ForbiddenSyntax(1001) InvalidLiteral`。
- 分类：[粗粒度] 中（静默补齐/截断掩盖错误输入）。

## 缺口（sci trap，sars 静默）

- sars `parse_header.rs::const_value` 对 `struct { a: N = <literal> }`：字面量字节数不等于 N 时 `bytes.resize(N,0)` + `truncate(N)`，**静默补齐/截断**，接受畸形输入。
- 对 `hex:\xNN`：遇到 `\x` 但后两位非合法十六进制时，fallback 把反斜杠当普通字节塞入，**静默接受**。
- sci 对以上及 utf8 未知转义一律报 `ForbiddenSyntax(1001) InvalidLiteral`（实测确认）。

## fail-before / pass-after

用例（不入库）：
```
@const BAD = struct { a: 4 = hex:\x01\x00 }   # 声明 4 字节，实际 2
@const BAD = struct { a: 2 = hex:\xZZ }        # 非法 hex 转义
```
- **before（`424c42b`）**：sars `rc=0` 静默通过；sci `rc=1` `ForbiddenSyntax(1001)`。
- **after（本提交）**：sars `rc=1` `E1001 ForbiddenSyntax: InvalidLiteral`，与 sci 同 trap 码、同消息、同行号（1:1）。

## 实现

- `crates/sa-parser/src/parse_header.rs::const_value`：
  - struct 字段：声明 size 与字面量长度不等 → 返回 `None`（不再 resize/truncate）。
  - hex：遇到 `\x` 后两位非法或越界 → 返回 `None`（不再塞原字符）。
- `crates/sa-parser/src/parser.rs` 的 `@const` 分支：解析失败诊断从通用 `ParseError(2001)` 改为 `ForbiddenSyntax(1001) InvalidLiteral`，对齐 sci 对全部非法 const 的统一口径（含 C1 utf8 未知转义）。
- 更新既存 `tests/c1_const_utf8.rs`：断言新契约（`ForbiddenSyntax` + `InvalidLiteral`），旧的 "invalid constant value" 消息断言已过时。

## 证据与回归门

- **单元测试**：`sa-parser` lib 25 passed（新增 `malformed_const_literals_are_rejected`）；C1 集成测试按新契约更新。
- **全量**：`cargo test --workspace` 305 passed，0 failed。
- **合法 const**：rosetta 29_const_data / 962_mut_borrow_code / 107_refcell_dynamic_borrow 均 `rc=0`，无误伤。
- **968-name rosetta check**：与基线 `424c42b` 逐项一致——`rc=0 count=962 sha256=830304a687cf6de…`、`rc=1 count=6 sha256=8f89c17e76e9…`，零回归。

## 备注

这是 parser 层改动（非 verifier pre-pass）。sci 用 ForbiddenSyntax(1001) 覆盖所有 const 字面量错误；本批统一了 sars 的契约，使 C1（utf8 严格）与 B14（struct/hex 严格）在 trap 口径上一致。
