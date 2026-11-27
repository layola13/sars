# D1 — fallible 状态模型（对标 sci armTry / armReturn）

状态：已落地，968 名零回归，默认/legacy 逐字节一致。基线 HEAD `2462b65`。

## 语义收敛

- `Try`（`rules.rs`）：sci `armTry` 的契约是 `(src & fallible) != 0`。旧 sars 仅当 `fallible_get(src) == Some(false)` 才报错，`None`（未由 call 产生）被当作可 `?`。现改为 `fallible_get(src) != Some(true)` 即报 `FallibleContractMismatch`——只有 call 结果才是 fallible。
- `Return`（`rules.rs`）：新增 sci `armReturn` 检查——返回 fallible 值但当前函数签名 `return_fallible == false` 时，报 `FallibleContractMismatch`（要求用 `?` 传播或从 fallible 函数返回）。
- 两处均经 `RegStateBackend`，`FnState` 与 `CapabilityState` 双后端行为一致。

## 证据

- fail-before：在基线 `rules.rs` 上跑新测试，`d1_try_rejects_non_fallible_source` 与 `d1_return_rejects_fallible_from_nonfallible_function` 均 `FAILED`（`baseline_rc=101`）；接受侧两项通过。
- pass-after：`cargo test --test d1_fallible -p sa-verify` 4/4 通过，双后端各跑一遍。
- 968 名 rosetta `sars check`：rc=0 962 / rc=1 6，sha256 `830304a6…` / `8f89c17e…` 与基线逐字一致——收紧 Try 未翻转任何 demo（语料中 47 个含 `?` 的文件，其 `?` 源均为 call 结果）。
- 默认 vs `--legacy-state`：968/968 stdout+stderr+rc 逐字节一致（diff=0）。

## 边界

- 本轮只收敛 verifier 侧 D1；解释器侧 fallible（清单 interp A1/A2：`Frame.fallible: HashSet` 按名一次性 set、`?` 对 null 误判）属独立项，未触碰——避免在 demo 372 等冻结面上翻转。
- `EarlyReturn`/`Panic` 的 fallible 泄漏检查属 D2（EarlyReturnLeak），未并入本轮。
- 未触碰冻结面：R1 回边、JS callback/float/codeval、81 sa_tmp、963/964 校验值。
