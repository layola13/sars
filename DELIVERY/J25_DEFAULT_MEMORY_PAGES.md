# J25 — JS 默认内存页数（对标 sci JsEmitOptions.mem_pages）

状态：已落地，968 check 与默认/legacy parity 零回归。基线 HEAD `5c2e1de`。

## 语义收敛

- sci `emit_js/types.zig:21`：`JsEmitOptions.mem_pages = 256`（256×64KiB = 16MiB 初始）。
- sars 旧 `JsOptions::default().memory_pages = 2`，与 sci 默认值不符；CLI `build-js` 虽显式传 256（`commands.rs:180`），但库调用方拿到的是 2 页。
- 本次把默认值改为 256，并更新两个固化为 2 页的单元测试断言（`default_options_match_spec`、`empty_program_emits_valid_esm` 的 `SA_PAGES`）。
- `sa-emit-wasm` 的默认 2 页未改（sci 侧 wasm 默认另属一套，清单 J25 只针对 JS）。

## 证据

- fail-before：在基线 `lib.rs`（默认 2）上跑更新后的测试，2/2 `FAILED`；`baseline_rc=101`。
- pass-after：`cargo test --workspace` rc=0（sa-emit-js 21+2 测试全绿）。
- 968 check：rc=0 962 / rc=1 6，sha256 `830304a6…` / `8f89c17e…` 与基线逐字一致。
- 默认 vs `--legacy-state`：968/968 逐字节一致（diff=0）。

## 边界

- 只影响 JS 发射器的 `JsOptions::default()`；CLI build-js 已显式 256，行为不变。
- 未触碰冻结面：R1 回边、JS callback/float/codeval、81 sa_tmp、963/964 校验值。
