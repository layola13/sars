# B2 — FFI airlock 上下文门禁（对标 sci armRawCast/armAssumeSafe/armAssumeBorrow）

状态：已落地，968 check 与默认/legacy parity 零回归。基线 HEAD `31adabd`。

## 语义收敛

- sci 在 `verify_body_arms_assign_branch.zig:105,112,130` 对 `RawCast`/`AssumeSafe`/`AssumeBorrow` 开头检查 `current_is_ffi_wrapper`，非 ffi_wrapper 直接 `trapReport(.illegal_unsafe_context)`：「raw pointer and assume_* instructions are only legal inside @ffi_wrapper」。
- sars 旧行为：这三类落 default 或与 Borrow 共用臂，无任何上下文门禁，`Trap::IllegalUnsafeContext`（code 1019）已定义但 verifier 从不发。
- 本次在 `rules.rs::transfer` 入口加门禁：`RawCast`/`AssumeSafe`/`AssumeBorrow` 且 `ctx.signature.kind != FunctionKind::FfiWrapper` → report `IllegalUnsafeContext` 后 return（对齐 sci 的 early trap）。普通 `Borrow` 不受影响。

## 证据

- 语料统计：6 个 demo 含 `@ffi_wrapper`，4 个含 `assume_*`（182/184/185/193）全部在 ffi_wrapper 函数内；`RawCast`（`= *x` 指令形式）0 处。收紧不翻转任何 demo。
- fail-before：在基线 `rules.rs` 上跑新测试，3 个「外部使用应拒绝」`FAILED`（基线返回 Ok），2 个「允许」测试通过；`baseline_rc=101`。
- pass-after：`cargo test --test b2_ffi_airlock -p sa-verify` 5/5（双后端）；`cargo test --workspace` rc=0。
- 968 check：rc=0 962 / rc=1 6，sha256 `830304a6…` / `8f89c17e…` 与基线逐字一致。
- 默认 vs `--legacy-state`：968/968 stdout+stderr+rc 逐字节一致（diff=0）。

## 边界

- 门禁在 `transfer` 入口统一处理，三类指令后续的具体规则（如 AssumeBorrow 与 Borrow 共用臂的借用逻辑）在合法上下文下不变。
- `Trap::IllegalUnsafeContext` 的 code 1019 与 sci 一致。
- 未触碰冻结面：R1 回边、JS callback/float/codeval、81 sa_tmp、963/964 校验值。
