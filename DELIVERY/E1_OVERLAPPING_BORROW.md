# E1 第二批 — 重叠 interior 字段的可变借用检查（sci parity）

- 基线：`8673eaa`（E1 第一批）；落地提交见下方提交号。
- 对照 sci：`hasOverlappingInteriorBorrow`（`snapshot_merge_state_in_scope.zig:405-414`）、`armBorrow` 的 mut 分支（`verify_body_arms_alloc_borrow.zig:211-239`）、`setInteriorOffsetFromBase`（`:368-392`）、`setBorrowState`（`collect_metadata.zig:617-630`）。
- 分类：[能力缺失] 高（E1 协议剩余的主体部分）。

## 缺口（sci trap，sars 静默）

sci 把 `ptr_add` 派生的指针登记为 interior 字段，身份 = `(interior_root, interior_offset)`，偏移沿派生链累加；`&` 借用把锁记在**源寄存器**上（`locks[src] += 1`，`state[src] = locked_read/locked_mut`）。对某字段做 `&mut` 时，若有**另一个**寄存器在同一字段上持活跃借用，sci 报 `ReadWriteConflict(1011)` "cannot borrow mut while overlapping field borrows are active"。

sars 的 `add_borrow`（`rules.rs:237-344`）只按单个源寄存器名判断 shared/mut，完全不追踪字段身份——同一 alloc 的两个 `ptr_add` 字段被视为无关，漏报重叠借用。

## fail-before / pass-after

用例 `.eval/e1_ov3.sa`（不入库，`.eval/` 已 gitignore）：

```
buf = alloc 16
f0 = ptr_add buf, 0
f1 = ptr_add buf, 0
r0 = &f0
r1 = &mut f1
```

- **before（`8673eaa`）**：sars `rc=0` 静默通过；sci `rc=1` `ReadWriteConflict(1011)`。
- **after（本提交）**：sars `rc=1` `E1011 ReadWriteConflict: cannot borrow mut while overlapping field borrows are active`，与 sci 同 trap 码、同消息、同行号（7:5）。

对照用例：
- `.eval/e1_ov2.sa`（同寄存器 `&f0` 后 `&mut f0`）：仍只由主数据流 `add_borrow` 报 `E1008 BorrowConflict`，本 pre-pass 跳过源寄存器自身，无重复诊断。
- `.eval/e1_ov_distinct.sa`（`f0=+0`、`f1=+4`，不同字段）：sci/sars 均 `rc=0`，无误伤。

## 实现

`crates/sa-verify/src/bounds.rs` 新增 `check_overlapping_borrows`（batch 2），与 batch 1 同样是**线性函数内 pre-pass**，不侵入 `RegStateBackend`/merge：

- `fields: reg -> (root, off)`：由 `Alloc` 根经 `PtrAdd` 按静态偏移累积建立（对齐 `setInteriorOffsetFromBase`）；`Move` 传播字段身份。
- `locks: src_reg -> (mutable, field)`：借用锁记在源寄存器（对齐 `locks[src]`）；`&mut` 覆盖、shared 累加。
- `view_origin: view -> src`：`!view`（Release）时解锁源（对齐 `origins`/clearBorrowState）。
- `&mut` 时遍历其他持锁寄存器，`same_field`（同 root + 同静态 offset）即报 `ReadWriteConflict`。源寄存器自身跳过（交主 verifier），动态/未知偏移一律宽松（`okrelse` 原则，避免误伤 968）。

接入点 `lib.rs::run_static_bounds` 每函数调用一次两个 pre-pass。`Trap::ReadWriteConflict(1011)` 此前在 sars enum 中定义但从未发出，本批首次激活。

## 证据与回归门

- **单元测试**：`sa-verify` lib 36 passed / 0 failed（含 4 个 batch2 用例：重叠拒绝、异字段放行、同寄存器不重复、release 后不冲突）。
- **全量**：`cargo test --workspace` 全绿，0 failed。
- **968-name rosetta check**：与基线 `8673eaa` 逐项一致——`rc=0 count=962 sha256=830304a687cf6de…`、`rc=1 count=6 sha256=8f89c17e76e9…`，零回归。

## 范围更正（对 #18819/#18822 的撤回）

此前判断"E1 剩余需改 parser+IR 让 borrow 源带偏移"是**错误**的：实测 sci 自身也不支持 `&mut buf+0`（报 `UnknownRegister: buf+0`），sars 同样。字段身份来自 `ptr_add` 建立的 interior tree，不在源码语法层。本批因此不改 parser/IR，仅改 verifier pre-pass。

## E1 收尾状态

E1 两批（静态越界 `8673eaa` + 重叠借用本批）覆盖了 `checkStaticOffsetInBounds` 与 `hasOverlappingInteriorBorrow` 两个可源码复现的核心检查。sci interior tree 的更深状态（跨分支 merge 精度、take/load 的 interior child attach）在公开 `.sa` 语料层无干净 fail-before，按"先证据再实现"纪律不在无用例情况下实现。下一项按队列接 `E4`（store 写检查）或 muse 指定。
