# B5-fix：transfer 检查序对齐 sci（#19691 终局裁定）

- 日期：2026-11-28 14:4x CST；执行：workbuddy（25027 中继巡视）
- 裁定依据：#19691（主会话代裁、终局）——6fd0721 保留不回滚，改 fix-forward；
  立 B5-fix 批叠在 df231d8 上；ACTIVE 检查移到 LOCKED 之前；真值表 0x02/0x04
  的 TransferOwn 行 trap 同步改 UseAfterMove；case8b/case9 为 fail-before、
  case8/case10 为对照组；全门照旧；本地 commit 报 muse 核验代推。
- Oracle：sci `zig-out/bin/sa` + `src/verifier/verify_body_arms_alloc_borrow.zig:251-302`
  armTransfer（line 276 `(cmask & ACTIVE)==0 -> use_after_move` 在
  hasActiveBorrowRefs 之前）。

## fail-before（干净复现，sci JSON 证据）

| 用例 | 形态 | sars before | sci oracle | 判定 |
|---|---|---|---|---|
| case8b | `b=&x` → x=LOCKED_READ 0x02 | E1008 BorrowConflict | E1009 UseAfterMove (actual_mask=2) | 分叉 |
| case9 | `b=&mut x` → x=LOCKED_MUT 0x04 | E1008 BorrowConflict | E1009 UseAfterMove (actual_mask=4) | 分叉 |
| case8_legal | 合法 transfer | E1012 leak(x) | E1012 leak | 对照组一致 |
| case10 | `^x` → x=CONSUMED 0x08 | E1009 UseAfterMove | E1009 UseAfterMove | 对照组一致 |

复现件：`/tmp/b5fix_probe/case{8b,9_active_borrow_mut,8_legal,10_consumed}.sa`（仓外）。

## 改动（4 文件）

- `crates/sa-verify/src/rules.rs`：Transfer 臂把 `ACTIVE==0 -> UseAfterMove` 检查
  移到 `LOCKED_* -> BorrowConflict` 之前；保留 active|locked 组合态的
  borrow_conflict 分支（sci hasActiveBorrowRefs 语义，不误伤）。
- `crates/sa-verify/src/mask.rs`：TRUTH_TABLE 的 0x02/0x04 TransferOwn 行
  trap 由 BorrowConflict 改 UseAfterMove。
- `crates/sa-verify/src/tests.rs`：transfer_with_active_borrows 断言改
  UseAfterMove；真值表 0x02/0x04 断言同步。
- `crates/sa-verify/tests/a1a_state_carrier.rs`：28 行 expected 数组里
  0x02/0x04 TransferOwn 行同步为 UseAfterMove。

## pass-after / 全门

- 双边复验：case8b/case9 sars E1009 == sci trap_code 1009；case8_legal
  E1012 / case10 E1009 对照组不变。
- cargo test --workspace：353 passed / 0 failed。
- 968 rosetta：rc0=962 sha 830304a6…2922、rc1=6 sha 8f89c17e…d55，
  与 cf1a275/df231d8 基线双 sha 完全一致，零翻转。

## 附带核查（裁定③：单独登记，不扩围）

- 探针 `/tmp/b5fix_probe/lifecycle_probe.sa`：`b=&mut x` 后不 release 直接
  返回。sars check rc=0 静默放行（x 停在 LOCKED_MUT 0x04，
  finalize_leaks 不计 LOCKED 态为 live）；sci 报 MemoryLeak 1012
  （actual_mask=4 Locked_Mut）。确属 LOCKED 无 ACTIVE 经非 transfer 路径
  （borrow 未配对 release）到达函数出口的生命周期漏报，为裁定③预见的
  真生命周期缺陷形态。按裁定只登记、不在本批扩围，报 muse 单列。

## 红线遵循

- 6fd0721 未回滚；B4/A2/B7 证据提交 96f51a9 未触碰；B6（285073d）仍未解冻，
  待 B5-fix 核验后按 #19691④ 变基续推；A3/E2 冻结未触。
- 本地 commit，未 push（报 muse 核验代推 #19691②）。
