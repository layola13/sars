# RESULT — SIMPLIFICATION_GAPS 续推（首报 + 首项分叉登记）

- 时间：2026-11-26 14:21–14:24 CST / 06:21–06:24 UTC
- 工作副本：`/content/oc-box-1-work/sars`
- 任务分支：`task/simpl-gaps-a1`，基点 `8b51475695d23881b78baa82a816b396cef7e99b`
- 清单正本：盒上 4 份副本 md5 全为 `a4ce38a26214fe0c377f778c45e13eed`，248 行、146 项。
  盒上无 `~/workspace/sars-audit` 挂载；主会话若已更新正本请贴差异/新版。

## 1. C 计数对账（已清）

| 读数 | 来源 | 判定 |
|---|---|---|
| 962/968 | 已入账、`8b51475` 提交文案、`c_corpus_env_fail_before_pass_after.log` after_rc0 | **正确，保留** |
| 900/968 | 我 09:40 引用的是 `c_vtable_fail_before_pass_after.log` 的 after_rc0（`24a2481`） | 旧档误读，作废 |

- fresh recompute：用当前 HEAD `8b51475` 的 release 二进制
  （sha256 `a1367da1488f335558792a7f889adc658f4be1ca34bba3d9cc274bd7ababf9d8`，
  与 `C_CORPUS_ENV.md` 登记 after 一致），对 968 名 rosetta 逐个 `build-exe` + 实跑，
  single-pass 51.6 s → `total=968 pass_build_and_run=962`。
- 绿名集与已入账日志逐字一致（fresh-only=[]、doc-only=[]）；红 6 名
  `205/207/226/227/243/502` 全为负例（build_rc=1，按设计应失败），pass→fail=0。
- 演进链：`745(244f5ed) → 788(35518dc) → 900(24a2481) → 930(42704ea) → 931(e521f98) → 962(8b51475)`。
- 证据：`DELIVERY/c_fresh_recompute_20261126.log`；脚本 `/tmp/opencode/c_recompute.py`。

## 2. 首项（清单价值序自上而下）= §1 A1「寄存器状态模型」

- 清单原文：`RegState`/`RegKind` 裸 enum + 并行 HashMap（`state.rs:3-33`），
  sci 为 12 位 `CapabilityMask`（`common/capability.zig:3-19`）＋真值表（`:74-100`）；
  分类 `[简化模型] 高：下游多数缺口根因`。
- 完整语义范围实测：sci 侧触及 mask/位语义的 verifier/referee/common 源码约 **6,925 行**
  （9+ 文件：`reg_state_change.zig`、`clear_borrow_ctx.zig`、`verify_body_arms_alloc_borrow.zig`、
  `const_trap.zig`、`collect_metadata.zig`、`verify_body_arms_call_return.zig`、
  `snapshot_merge_state_in_scope.zig`、`referee/table.zig`、`common/capability.zig`）；
  sars 侧 sa-verify 全量仅 3,798 行，其中 74 处 `rules.rs` 状态操作、31 处 `state.rs`。
- A1 不是语料名集簇，没有独立 fail-before/pass-after 名集；其全部可观测后果分别就是
  A2（phi/合并格）、A3（escaped join）、B1（capability 空壳）、B2（FFI airlock）、
  B3（@const 不可变）、E1（interior pointer）、D1（fallible 状态）等后续项。
  单独给 A1 一个「简化版 mask」冒充打平，正违反禁止简化令。

## 3. 分叉（停该项，等主会话裁定，不自磨、不降级）

请选择其一：

1. **A1 作为整块 verifier 重写一项交付**：照 sci 12 位 mask + 真值表逐位落地，
   连带 A2–I5 的状态语义一次性收口；工作量大、跨多会话，期间 verifier 线不再并开新项。
2. **把 A1 拆成有独立验收的子项**（例如先 mask 类型与真值表骨架、再逐位接入规则），
   每项给单独的 fail-before/pass-after 单元证据后再开工。
3. **A1 保持 open 登记，先跳到清单中下一个有独立名集/独立验收的项**；需主会话指定起点。

另有两项请先确认：
- 本机主本 `~/workspace/sars-audit/SIMPLIFICATION_GAPS.md` 若比盒上副本新，请贴新版或差异；
  盒上当前正本为 248 行 / 146 项 md5 `a4ce38a2…`。
- 清单 §10 的重新定界请求与 R1 A/B/C、JS callback/float/codeval、81 sa_tmp、963/964
  冻结登记：本次派单是否视为重新定界并解冻这些项？未裁定前一律不开工。

## 4. 本笔状态

- 未改任何实现代码；未降级、未静默跳过；A1 停在分析/分叉上报。
- 中继第一报 id `12963`（C 对账），本笔 RESULT 随提交落盘。
