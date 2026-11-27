# A15 — 解释器 atomic_rmw min/max 归约语义（对标 sci）

状态：已落地，968 check 与 968 run 均零回归。基线 HEAD `aa7949a`。

## 缺陷与修复

- 缺陷（清单 interp A15）：`exec.rs::rmw_op` 把 `Umin/Umax/Smin/Smax` 映射到 `OpKind::Ult/Ugt/Slt/Sgt`（比较 op），`apply_op` 的比较分支返回布尔 0/1，于是 `atomic_rmw_umin ...` 把 **0 或 1** 写回内存，而非两个操作数中的较小/较大者。这是真实语义 bug，不只是精度问题。
- 修复：新增 `rmw_minmax(op, old, rhs, ty)`，对标 sci `atomicRmwApplyImpl`（`src/interp/interpreter_init.zig:383-405`）：
  - `Umin/Umax` 按 u128 无符号比较，`Smin/Smax` 按 i128 有符号比较；
  - 返回被选中的操作数本身，按指令类型位宽 `mask_i128` 归一（unsigned 保留无符号表示，signed 符号扩展）；
  - `rmw_op` 对 4 个 min/max 标 `unreachable!()`，防止再次误走比较路径；其余 op 映射不变。

## 证据

- fail-before：在基线 `exec.rs` 上跑新测试，4/4 `FAILED`（比较 op 写回 0/1）；`baseline_rc=101`。
- pass-after：`cargo test --test a15_atomic_minmax -p sa-interp` 4/4 通过；`cargo test --workspace` rc=0。
- 968 check：rc=0 962 / rc=1 6，sha256 `830304a6…` / `8f89c17e…` 与基线逐字一致。
- 968 run（新旧二进制逐名对拍 rc+stdout）：807/160/1 分桶，`flips=0`——无任何 demo 输出变化。
- 默认 vs `--legacy-state` check 输出：968/968 逐字节一致（diff=0）。
- 语料扫描：968 个 rosetta demo 中无 `atomic_rmw_*min/max` 用法（仅 `109_atomic_fetch_add` 等 add 族），故本修复不触碰任何现有 demo 语义。

## 边界

- 未触碰冻结面：R1 回边、JS callback/float/codeval、81 sa_tmp、963/964 校验值。
- 解释器其余已知问题（fallible 按名 set、null `?` 误判、i128 算术、扁平内存等）属独立清单项，未并入本轮。
