# A2 分支合并冲突硬诊断（sci parity，收窄口径）

- 日期：2026-11-28；批次父提交：fdcd8c5（B4）
- oracle：sci `verify_body_arms_assign_branch.zig:154-260` 与
  `snapshot_merge_state_in_scope.zig:543` phiStateConflictReport（Trap 1015）

## fail-before / pass-after

- 用例 `/tmp/a2_audit/case1_join_conflict.sa`：分支 L_A 释放 x、L_B 不释放，
  两条路径汇入 L_JOIN。
  - before（fdcd8c5 的 release 二进制）：sars rc=0 静默（PhiStateConflict 仅为 warning，
    不影响退出码）；sci rc=1 `PhiStateConflict` 1015，`incoming control-flow states do not agree`，
    定位第 10 行的 jmp。
  - after：sars rc=1 `[E1015] PhiStateConflict: incoming control-flow states do not agree`，
    `case1_join_conflict.sa:10:5`，与 sci 的 trap 名/码/报文/行号一致。
- 双边负例：rosetta `314_tail_call_become` 在 sci rc=0；其 sars 合并冲突是
  `Owned{Param}` 与 `Owned{Scalar}` 的回边重绑定（sci PATCH join 容忍），本批不将其升级为错误，
  968 名集与双 sha 保持基线不变。

## 改动

- `crates/sa-verify/src/lib.rs`：phi 合并冲突不再无条件只发 warning。
  - 当冲突形状为「一侧 Moved/Released、另一侧 Owned（含 Alloc 等）」时，
    发 Trap::PhiStateConflict 硬错误，报文用 sci 原文，位置取当前块终止指令（jmp/br）的行号。
  - 其余粗粒度冲突（如回边重绑定）保持原 warning 文案与路径，避免误伤。
  - `phi_warning` 接收当前块终止位置，避免把冲突错误挂在汇入块末行。
- `crates/sa-verify/src/tests.rs`：两条单测验证消费-存活汇入为错误；
  原 warning 用例按新语义改名/改断言。

## 门

- `cargo test --workspace`：41 套件，355 passed / 0 failed。
- `python3 .eval/sars_968.py target/release/sars`：total=968，
  rc=0 962 / sha256 830304a6…e2922；rc=1 6 / sha256 8f89c17e…d55，
  与 cf1a275 基线逐项一致，零翻转。

## 余项登记

- A2 全量快照（state + origins + locks + flags + interior tree + offsets + alloc sizes）
  与带 PATCH 的 join lattice 尚未实现；本批只覆盖有干净 fail-before 的
  「一侧消费、另一侧仍存活」形状。其余冲突仍为 warning，按证据纪律不硬造硬错误。
- 本批次只本地 commit，未推；按 #19312 报 muse 核验代推。
