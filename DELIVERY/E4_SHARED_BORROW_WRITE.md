# E4（首批）— 通过共享借用写检查（sci parity）

- 基线：`2a0f145`（E1 第二批）；落地提交见下方提交号。
- 对照 sci：`writeCheck`（`collect_metadata.zig:477-514`）由 `armStore`（`verify_body_arms_alloc_borrow.zig:82-102`）调用。
- 分类：[能力缺失] 中（E4 store 写检查的可源码复现子集）。

## 缺口（sci trap，sars 静默）

sci 对 `store addr` 的 `addr.base` 调 `writeCheck`。当 addr 是 borrow view、且其 origin 有**多个**活跃共享借用时，报 `ReadWriteConflict(1011)` "cannot write through a shared borrow"（`writeCheck:500-506`）。单个共享借用仍允许一次"物化"写，并把 origin 升级为 `locked_mut`（`armStore:92-98`）。

sars 的 Store 分支（`rules.rs:756-763`）只做 `read_operand` + `mark_escaped_alloc` + define，**完全没有写权限检查**——通过任意共享借用写都静默放行。

## fail-before / pass-after

用例 `.eval/e4_shared_write2.sa`（不入库）：

```
buf = alloc 4
v0 = &buf
v1 = &buf
store v0+0, 9 as i32
```

- **before（`2a0f145`）**：sars `rc=0` 静默；sci `rc=1` `ReadWriteConflict(1011)`。
- **after（本提交）**：sars `rc=1` `E1011 ReadWriteConflict: cannot write through a shared borrow`，与 sci 同 trap 码、同消息、同行号（6:5）。

对照：
- `.eval/e4_shared_write.sa`（单个 `v0=&buf` 后 store）：sci/sars 均 `rc=0`（单读者可物化写），无误伤。

## 实现

`crates/sa-verify/src/bounds.rs` 新增 `check_shared_borrow_writes`（第三个 pre-pass，仍不动 `RegStateBackend`/merge）：

- `view_origin: view -> origin`：`&`/`&mut` 的 dest 记到源。
- `shared_count: origin -> 数`：共享借用计数；`mut_origins: set`：独占借用。
- `Release` 时按 view 找 origin 解锁（先 mut 后 shared）。
- `Store` 时若 `addr.base` 是某 view、其 origin 的共享读者数 `>1` → 报 `ReadWriteConflict`。单一读者与独占借用放行（对齐 sci）。

接入点 `lib.rs::run_static_bounds` 每函数调用一次三个 pre-pass。`Trap::ReadWriteConflict(1011)` 在 E1 第二批首次激活后，本批复用。

## 证据与回归门

- **单元测试**：`sa-verify` lib 39 passed / 0 failed（新增 3 个 E4 用例：多读者写拒绝、单读者写放行、释放后放行）。
- **全量**：`cargo test --workspace` 41 套件 / 304 测试全绿，0 failed。
- **968-name rosetta check**：与基线 `2a0f145` 逐项一致——`rc=0 count=962 sha256=830304a687cf6de…`、`rc=1 count=6 sha256=8f89c17e76e9…`，零回归。

## 备注

E4 余下子项（immutable const 的 store 写）在源码层无法构造干净 fail-before：sci 对 `&CONST` 的写因独占借用授权而放行（`writeCheck:493-498`），`@const` 直接作 store 目标则是 `InvalidLiteral` 语法错而非语义 trap。按"先证据再实现"纪律不在无用例情况下实现。本批已覆盖 E4 中可源码复现的核心缺口。
