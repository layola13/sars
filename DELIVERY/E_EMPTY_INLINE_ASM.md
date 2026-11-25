# E 簇 — JS 空内联汇编降为 no-op（回 10754 ④ 落地序 D→E→…）

## 根因与修复（非简化，与 sci 同判据）
`crates/sa-emit-js/src/lower.rs` 对 `InstructionKind::Native` 一律发 `UnsupportedInstruction`，
而 sci 的 `src/emit_js/dispatch.zig:363-371` 对空模板 `asm sideeffect ""` 降级为 no-op，
非空 native escape 仍响亮 trap。修复按同一判据：`text.trim().contains("asm sideeffect \"\"")` 时发射 no-op，
否则保持原具名 Diagnostic（禁止静默 0/noop 冒充非空 asm）。

新增单测 `empty_inline_asm_is_a_no_op`（空 asm 成功发射且 node 跑通）；原 `native_instruction_is_rejected`（非空仍拒）保持。

## 证据（973 JS 名；before = D 簇后二进制 e0ab3a2，after = 本笔）
- fail-before：`117_inline_assembly` build-js rc=2001，
  `[E2002] UnsupportedInstruction: native instruction is not supported by the JS backend`。
- pass-after：rc=0、stdout `7\n`；sci 参考同输出 `7\n`（逐名对拍一致）。
- **before 942/973，after 943/973**；**pass→fail = 0**；fail→pass = 1（`117_inline_assembly`）。
- `cargo test --workspace` 全绿（209/209）。取证件：`e_fail_before_pass_after.log`、`e_harness.py`。
