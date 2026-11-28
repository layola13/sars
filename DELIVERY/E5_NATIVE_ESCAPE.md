# E5 — native escape 边界消费（sci parity）

- 基线：`4ff06f7`（E3）；落地提交见下方提交号。
- 对照 sci：`armNative`（`verify_body_arms_assign_branch.zig:141-151`）遍历 native 引用的每个寄存器，调 `consumeAtContractBoundary`（`clear_borrow_ctx.zig:131-153`）。
- 分类：[能力缺失] 中（native 边界的按引用消费与栈逃逸拒绝）。

## 缺口（sci trap，sars 静默/错 trap）

sci 对 `$...$` native 行，从文本提取寄存器名（`collectNativeRegisterNames`, `line_classifier_parse.zig:204`：跳引号串、跳 `@`/`%` 前缀、去重），对每个已声明寄存器：先 read/borrow 检查，若为栈分配报 `StackEscape(1025)` "stack allocation cannot cross a native escape boundary"，否则 move-消费（置 consumed）。

sars 的 `Native` 指令此前落入 `rules.rs` 默认分支，只 `read_operand` + define Value：既不消费引用寄存器，也不拒绝栈逃逸。

## fail-before / pass-after

用例一 `.eval/e5_native_stack.sa`（栈逃逸，主 fail-before）：

```
@main() -> i32:
L_ENTRY:
    tmp = stack_alloc 8
    $touch tmp$
    return 0
```

- **before（`4ff06f7`）**：sars `rc=0` 静默；sci `rc=1` `StackEscape(1025)` "stack allocation cannot cross a native escape boundary"。
- **after（本提交）**：sars `rc=1` `E1025 StackEscape: stack allocation cannot cross a native escape boundary`，与 sci 同码同消息。

用例二 `.eval/e5_native_consume.sa`（消费一致性）：

```
@main() -> i32:
L_ENTRY:
    value = alloc 8
    $touch value$
    probe = load value+0 as i8
    return 0
```

- **before**：sci `rc=1` `UseAfterMove`（native 已消费 value）；sars `rc=1` `MemoryLeak`（未消费）——同 rc 不同 trap。
- **after**：sars `rc=1` `E1009 UseAfterMove: moved value 'value' is no longer usable`，与 sci 同 trap。

对照：968 rosetta 中唯一 native 用例 `117_inline_assembly`（`$call void asm sideeffect "", ""()$`）文本无已声明寄存器 token，after 仍 `rc=0`，无误伤。

## 实现

`crates/sa-verify/src/rules.rs`：

- 新增 `native_reg_names(text)`：镜像 sci `collectNativeRegisterNames`，跳引号串内容、跳 `@`/`%` 前缀标识符、去重；返回文本中的标识符 token。
- `InstructionKind::Native` 从 default 提升为专门分支：对每个 token，仅当 `state_of(name)` 为已声明（非 Undefined/Conflict）才处理——`Owned{StackAlloc}` 报 `Trap::StackEscape`，其余经 `consume()` move-消费（内含 read/borrow 检查）。

栈逃逸消息文本与 E3 区分（"cannot cross a native escape boundary" vs "cannot be returned"），对齐 sci 两处 `trapReport`。`Trap::StackEscape(1025)` 复用 E3 已激活的定义。

## 证据与回归门

- **全量**：`cargo test --workspace` 41 套件全绿，0 failed。
- **968-name rosetta check**：与基线 `4ff06f7` 逐项一致——`rc=0 count=962 sha256=830304a687cf6de…`、`rc=1 count=6 sha256=8f89c17e76e9…`，零回归。

## 备注

A3/E2（escaped join 并集 / call arg 粗粒度 escaped）与本项同源但风险更高：sars 对 ByValue call arg 无差别 `mark_escaped_alloc`，sci 按 cap 精确消费；粗暴移除该标记会破坏 968（新增 247 分叉），需精确 cap 语义重构，暂不硬改。
