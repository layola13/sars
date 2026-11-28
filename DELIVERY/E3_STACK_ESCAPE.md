# E3 — 栈分配逃逸检查（sci parity）

- 基线：`d23ec1a`（B14）；落地提交见下方提交号。
- 对照 sci：`armReturn`（`verify_body_arms_call_return.zig:303-305`）的 `isStackAllocated` → `trapReport(.stack_escape)`。
- 分类：[能力缺失] 中（Return 路径栈分配按值逃逸）。

## 缺口（sci trap，sars 静默）

sci 在 `return` 的非 fallible、by-value 分支检查操作数是否为栈分配：`isStackAllocated(...)` 成立即报 `StackEscape(1025)` "stack allocation cannot be returned"（`verify_body_arms_call_return.zig:303-305,340-342`）。

sars 的 Return 分支（`rules.rs`）此前只做 `read_operand` + fallible 契约检查 + 常量按值移动 + `check_leaks`，**没有栈逃逸检查**——`@get()` 中 `return`（栈分配槽）静默放行。

## fail-before / pass-after

用例 `.eval/e3_stack_escape.sa`（不入库）：

```
@get() -> ptr:
L_F:
    slot = stack_alloc 8
    return slot
@main() -> i32:
L_ENTRY:
    p = call @get()
    !p
    return 0
```

- **before（`d23ec1a`）**：sars `rc=0` 静默（`OK: 2 functions, 5 instructions, gas used 12`）；sci `rc=1` `StackEscape(1025)` "stack allocation cannot be returned"，register=slot，line 5。
- **after（本提交）**：sars `rc=1` `E1025 StackEscape: stack allocation cannot be returned`，与 sci 同 trap 码、同消息、同行号（4:5 / source line 5）。

## 实现

`crates/sa-verify/src/rules.rs` 的 `InstructionKind::Return` 分支内新增：若操作数为寄存器、按值返回（`by_value`）、非 fallible，且 `state.state_of(name)` 为 `RegState::Owned { kind: RegKind::StackAlloc }`，则报 `Trap::StackEscape(1025)` "stack allocation cannot be returned"（reg=name，hint=None）。

- `Trap::StackEscape(1025)` 在 `sa-ir/trap.rs` 已定义（此前未被任何 verifier 路径激活），本批首次落地触发。
- 栈分配寄存器 `RegKind::StackAlloc` 由 `StackAlloc` 指令在 `rules.rs:724-731` 定义。
- 检查放在 fallible 契约检查之后、常量/移动处理之前，与 sci 的 `else` 分支顺序一致；`check_leaks` 只扫 `is_owned_alloc`（heap），不扫栈分配，故不产生额外 MemoryLeak 报。

## 证据与回归门

- **全量**：`cargo test --workspace` 41 套件全绿，0 failed。
- **968-name rosetta check**：与基线 `d23ec1a` 逐项一致——`rc=0 count=962 sha256=830304a687cf6de…`、`rc=1 count=6 sha256=8f89c17e76e9…`，零回归。

## 备注

E3 的其余逃逸路径（call 按值传参 `verify_body_arms_call_return.zig:140` "stack allocation cannot be passed by move"、native escape boundary `clear_borrow_ctx.zig:145`、显式 release `assign_branch.zig:92`）源码层 fail-before 需另行构造，本批先覆盖可源码复现的 Return 核心缺口。
