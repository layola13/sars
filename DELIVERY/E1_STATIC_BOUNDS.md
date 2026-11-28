# E1 第一批：静态偏移越界检查（sci parity）

- 日期：2026-11-28
- 基线：8c405af（B3 已落地头）
- 对照：sci `checkStaticOffsetInBounds`（`src/verifier/snapshot_merge_state_in_scope.zig:334-366`）
- 派单：muse #18803（先 fail-before/pass-after 证据、再改实现，每批报提交号）

## 范围（第一批）
只移植 sci 中**完全静态**的偏移越界检查：`alloc N` 的 size 与指针访问的字节偏移均为编译期常量时，访问落在 `>= N` 处报 `Trap::BorrowConflict`（码 1008）"pointer offset exceeds allocation size"。
- 动态偏移/动态 size、负偏移、来源不明的指针：保持宽松（与 sci 的 `static_offset orelse return null` / `!alloc_size_known` 一致）。
- interior tree 字段身份、重叠字段 borrow 检查（E1 剩余部分）：留待后续批次。

## 实现
- 新增 `crates/sa-verify/src/bounds.rs`：函数内线性 pre-pass `check_static_bounds`，记录静态 alloc size、指针视图（alloc 根 + 累积静态偏移），在 Load/Store/Take/PtrAdd/Atomic* 访问点越界则报错。
- 在 `lib.rs` 的 `verify` / `verify_function` 各入口通过 `run_static_bounds` 调用一次（独立于双后端数据流，不侵入 `RegStateBackend`/merge）。
- `rules.rs` 的 `RuleCtx::report` 由私有改为 `pub(crate)`，供 pre-pass 复用诊断构造。
- 关键教训：普通 `load` 得到的是独立指针值，不是 base 的 interior view；sci 仅在 source 带 borrow_view/interior_ptr flag 时才 attach interior child（`armLoad`, verify_body_arms_alloc_borrow.zig:75）。第一版错误地传播 view，导致 rosetta 131/164/776（vtable load）误报，已修正。

## fail-before / pass-after 证据

用例（`.eval/e1_oob.sa`）：
```
@main() -> i32:
L_ENTRY:
    buf = alloc 16
    x = load buf+16 as i32
    !buf
    return x
```

- **fail-before**（8c405af 二进制）：sars `check` rc=0 静默通过 `OK: 1 functions, 4 instructions`；sci `check` rc=1，报 `error[BorrowConflict]: pointer offset exceeds allocation size`（trap 1008）。→ 语义缺口坐实。
- **pass-after**（本批二进制）：sars rc=1，报 `.eval/e1_oob.sa:4:5 [E1008] BorrowConflict: pointer offset exceeds allocation size`，hint 与 sci 一致（`shrink the constant offset or widen the allocation`）。
- **边界内用例**（`.eval/e1_inbounds.sa`，offset 0/12）：sci rc=0，sars rc=0，无误伤。
- **单元测试**（`bounds::tests`，3 项）：oob 报 trap、inbounds 放行、动态偏移放行 —— 全部 ok。

## 回归门
- `cargo test --workspace`：41 测试套件全绿，0 failed。
- 968-name rosetta `sa check`：本批与基线 8c405af **逐项一致**
  - rc=0 count=962 sha256=830304a687cf6de03c1cb9650ba7e51051906a577918bb48a762e87b980e2922
  - rc=1 count=6 sha256=8f89c17e76e91f9538cebcdd5bbca74b00907be77ff8f0d2baace7adeb254d55
  - 对比用 `git worktree` 在 8c405af 构建基线二进制，同脚本 `.eval/sars_968.py` 对跑；零回归。
  - 过程中第一版曾因 view 误传播使 131/164/776 误报（959/9），修复后回到基线 962/6。

## 已知限制（后续批次）
- 仅静态偏移；基于 interior tree 的字段身份与重叠 borrow 尚未移植。
- `.eval/` 已 gitignore，评估脚本/日志不入仓。
