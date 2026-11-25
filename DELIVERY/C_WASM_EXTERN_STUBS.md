# C(wasm) extern 桩类型专项交付（回 10813 · 零页簇入账后继续）

- 排序：D→E→C(wasm)→C(JS)→F→零页簇→B；本笔只做 C(wasm)，不并簇。
- 裁定依据：10784/10792 的 C(wasm)[181/184 extern typed stub]；零页簇 `1e220fa` 已由主会话代推入账（JS 944/973、pass→fail=0，wasm 无 0 基址假设、不并簇），本机 HEAD 仍 `1e220fa`。

## 根因

sars 把 fd/pthread 调用按 `(env, name)` import 发射，R2 之前的通用零值桩对一切 extern 返回 0：
- 181：`fd_open` 返回 0 → `ok_fd = ne fd,0` 为假 → 走 `L_ERR`，stdout `error\n`、rc=1。
- 184：`pthread_spawn` 返回 0、`sa_pthread_join` 返回 0 → 句柄 0，worker 从未被调用 → `ok_value` 假 → 走 `L_ERR`，stdout `error\n`、rc=1。

sci 的同名 JS shim 实现的是真实 extern 契约（`src/emit_js/shims_misc_chan_mem_globals.zig:109-136,254-270`）：fd 句柄从 3 递增、`fd_read` 返回 3（rosetta oracle）、`pthread_spawn` 经 `__sa_ftable[i](arg)` **同步调用真实入口函数**并返回 ≥1 的 joinable 句柄、join/drop 返回 0。缺口在宿主桩类型，不在编译器发射——与 R2 同性质。

## 落地形态（完整契约，非简化）

仅改 `DELIVERY/typed_stub_runner.py`（新增 `mode="c"`，默认 `"r2"` 保持 R2 基线逐字不变）+ 新增 `DELIVERY/c_wasm_harness.py`。**编译器源码未动**。`C_DISPATCH` 在 R2 六类契约桩之上新增 6 个：

- `fd_open(path)`：从线性内存读 NUL 结尾路径，返回自 3 起单调递增的 fd；无 NUL 返回 `SA_STD_ERR_INVALID_ARGUMENT=1`。
- `fd_read(fd)`：校验句柄在册后返回 3（与 sci shim 同 oracle）；无效句柄返回 `SA_STD_ERR_INVALID_HANDLE=2`。
- `fd_close(fd)`：校验、注销、返回 `SA_STD_OK=0`。
- `pthread_spawn(entry, arg)`：sars 的 vtable 槽是 `SA_FN_BASE + program-function-index`（`crates/sa-emit-wasm/src/lib.rs:21,57`），只导出 main/memory。宿主解析 import/function 段后**仅在 export 段追加** `__sa_host_fn_<wasm_index>`（code/data/原有导出字节不变），解码 token 后用 `arg` **真实重入调用该 guest 函数**（与 sci `__sa_ftable` 同步入口同语义），返回自 1 起的 joinable 句柄；token 越界返回错误码，不静默 0。
- `sa_pthread_join(handle, out)`：句柄在册返回 0（入口已在 spawn 内跑完，单线程宿主无等待，与 sci shim 一致）。
- `pthread_drop(handle)`：注销、void。

没有硬编码任何 demo 输出；184 的 `5\n` 来自真实 guest worker 的返回值 5 与其写回的槽值。导出钩子只发生在宿主 runner，产物其余部分逐字节不变；无效 token/句柄一律具名 SA std 错误码，不是静默零值。

## fail-before / pass-after（969 全语料，stdout 口径）

同一编译器二进制 `/content/sa_all/sars/target/release/sars`（HEAD `1e220fa`），唯一变量是 `mode="r2"` → `mode="c"`：

| 指标 | before（R2 桩） | after（C 桩） |
|---|---|---|
| 全语料 | 969 | 969 |
| rc=0 | 924 | 926 |
| 2 名目标 rc=0 | 0 | 2 |
| stdout 逐字命中 | 0 | 2（`3\n` / `5\n`） |
| fail → pass | — | 2 |
| pass → fail | — | **0** |
| 原通过名的 stdout 变化 | — | **0** |

- fail→pass 逐名：`181_file_descriptor_raii` rc 1→0 / `error\n`→`3\n`；`184_pthread_spawn_join` rc 1→0 / `error\n`→`5\n`。两名的 Rust 源 `main.rs` 分别打印 3、5，stdout 与 oracle 逐字一致。
- 924 个原先 rc=0 的名字全部保持 rc=0 且 stdout 不变（含 R2 的 186/963/964：`8\n`/`ok\n`/`ok\n`），无 collateral。
- `182_mmap_memory_mapping` 亦 import `fd_*`，但 before/after 均 rc=1，不计入本簇、未做任何 mmap 桩扩面。
- `cargo test --workspace` **209/209** 全绿；无编译器源码改动，无回退风险。

## 证据

- `DELIVERY/typed_stub_runner.py`（C 模式实现；R2 模式保持原样）
- `DELIVERY/c_wasm_harness.py`、`DELIVERY/c_fail_before_pass_after.log`

## 后续

按序 C(JS)[181/184/186/963/964] → F[539_callback_codeval] → B[#npm_bind 诊断对齐 10 名]；build-exe 后续扩面（fallible ABI / vtable / atomics）另簇。本笔本地提交后请主会话代推并 PAT 核验，不停线。
