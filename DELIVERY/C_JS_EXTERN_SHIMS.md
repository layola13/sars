# C(JS) extern 桩类型专项交付（回 10840 · C(wasm) 入账后继续）

- 排序：D→E→C(wasm)→**C(JS)**→F→B；本笔只做 C(JS)[181/184/186/963/964]，不并簇。
- 裁定依据：10784/10792 的 C(wasm)[181/184 extern typed stub] 已由主会话验收并代推（`c6db242`）；本笔把同一「宿主桩是真实契约、不是零值桩」判据移植到 JS 后端，扩到 186(sqlite)/963(compress)/964(multipart)。
- 本机 before HEAD = `c6db242`，after = 本笔工作树；before 二进制快照 `/tmp/opencode/cjs_before/sars`。

## 根因

`crates/sa-emit-js/src/emit.rs::emit_footer()` 对一切未知 extern 发射通用零值桩
`SA_EXTERNS["<name>"] = function () { return 0; };`：
- 181：`fd_open`→0 → `ok_fd` 假 → `error\n` / rc=1。
- 184：`pthread_spawn`→0 → 真实 `@worker` 入口从未被调用 → `ok_value` 假 → `error\n` / rc=1。
- 186：`sqlite3_step`→0 → `ok_step = ne step,0` 假 → `error\n` / rc=1。
- 963：`sa_std_compress_supported`→0（bit0 未置）/ `encode`/`decode`→0 → `error\n` / rc=1。
- 964：`sa_std_multipart_parse`→0 → `error\n` / rc=2。

sci 的同名 JS shim 实现的是真实契约：
- `src/emit_js/shims_misc_chan_mem_globals.zig:109-136,254-270`（pthread 同步重入 `__sa_ftable`、fd 自 3 递增、`fd_read`→3）、`:360-371`（sqlite prepare→0 / step→100 / finalize→0）。
- `src/emit_js/shims_std_fs_compress_multipart.zig:20-127`（zlib gzip/deflate 真 round trip）、`:155-589`（multipart parse/builder/buffer 全状态机）。
- `src/emit_js/runtime_header_convert_registries.zig:337-343`（`__sa_node_builtin` 经 `process.getBuiltinModule` 取 node 内建）。
缺口在宿主桩，不在编译器发射——与 C(wasm)/R2 同性质。

## 落地形态（完整契约，非简化）

新增 `crates/sa-emit-js/src/shims.rs`（363 行，`emit_shim(name) -> Option<&'static str>`），在
`emit.rs:174-187` 的 extern 循环里优先取具名 shim、仅未命中才回退原零值桩；`lib.rs` 注册 module；
`tests.rs:345` 新增 `c_group_externs_get_real_contract_shims`。**不改 runtime.rs、不改通用回退路径**。

共 33 个具名 shim，逐字对齐 sci 的 JS 契约并改用 sars 运行库等价物（`__sa_addr`→`sa_num`、
`__sa_u8`→`U8`、`__sa_alloc`→`sa_alloc`、`__sa_store_i64`/`__sa_store_ptr`→`DV.setBigUint64`、
`__sa_fnval`/`__sa_ftable`→`sa_fnval`/`SA_FUNCS`、`__sa_node_builtin`→`process.getBuiltinModule`）：

- fd：`fd_open` 从 3 起单调递增、`fd_read`→3（sci 同 oracle）、`fd_close`→0。
- pthread：`pthread_spawn` 用 `sa_fnval(entry)` 解析 `SA_FUNCS[i]` 并 **真实同步重入 guest 入口**，
  返回自 1 起的 joinable 句柄；`sa_pthread_join`/`pthread_join`/`pthread_drop`→0。
- sqlite：`sqlite3_prepare`→0、`sqlite3_step`→100（SQLITE_ROW）、`sqlite3_finalize`→0。
- compress：`supported` 依 node:zlib 真置位、`encode`/`decode` 真调 gzip/deflate/brotli 并登记
  `globalThis.__sa_cbuf` 句柄表、`buffer_data`/`buffer_len`/`buffer_free` 按具名句柄取字节；
  无 zlib→8、坏格式→1、解码失败→6、不支持格式→8（与 sci 同错误码）。
- multipart：16 个 shim 全量移植 parse/count/part_*/builder_*/buffer_* 状态机，句柄表
  `globalThis.__sa_mp`，无效句柄→2/3、缺可选字段→100、容量不足→9（与 sci 同码）。
  184 的 `5\n` 来自真实 guest `@worker` 把槽值 1 加 4 后写回，963/964 的 `ok\n` 来自
  gzip/multipart 真 round trip 的逐字节校验；**没有任何 demo 输出硬编码**。
- sars 为 bump allocator、运行库 `sa_release` 本身即 no-op，故两个 `buffer_free` 只注销句柄表项、
  不调用 `__sa_free`（与 sars 底座语义一致，不引入悬垂释放）。

`no_comments`/`balanced` 单测保证发射文本无注释且括号配平；未知 extern 仍走原零值桩，
原 `externs_get_shims_and_calls_dispatch` 测试逐字不变。

## fail-before / pass-after（973 JS 全语料，stdout 口径）

同一 before/after 二进制，唯一变量是本笔 shim。取证件 `DELIVERY/c_js_harness.py`、
`DELIVERY/c_js_fail_before_pass_after.log`、`DELIVERY/c_js_baseline.log`（before vs before 自检）。

| 指标 | before | after |
|---|---|---|
| 全语料 | 973 | 973 |
| rc=0 | 944 | 950 |
| 2 目标 + 3 扩项 rc=0 | 0 | 5 |
| fail → pass | — | **6** |
| pass → fail | — | **0** |
| 原通过名 stdout 变化 | — | **0** |

- 5 个目标名逐名 stdout 与 sci 参考输出逐字一致：
  `181` `error\n`→`3\n`、`184` `error\n`→`5\n`、`186` `error\n`→`8\n`、
  `963` `error\n`→`ok\n`、`964` `error\n`→`ok\n`（rc 均 1/2→0）。
- **额外 fail→pass：`182_mmap_memory_mapping` `error\n`→`4\n`**。诚实记一笔：sars 未新增
  mmap/munmap shim（仍为通用零值桩），该名通过是 `fd_open`→3 使 `mapped=fd+1=4` 成立，
  叠加零页簇基址 65536 使读地址 0 得 0；输出 `4\n` 与 demo readme 明载 oracle 一致，
  但**不构成完整 mmap 契约**，报请主会话裁定是否计入本簇账（不隐藏、不冒充）。
- `cargo test --workspace` **210/210** 全绿（原 209 + 新增 1）。

## 证据

- `crates/sa-emit-js/src/shims.rs`（33 个 JS shim 源码）
- `crates/sa-emit-js/src/emit.rs:174-187`、`lib.rs`、`tests.rs:345`
- `DELIVERY/c_js_harness.py`、`DELIVERY/c_js_fail_before_pass_after.log`、`DELIVERY/c_js_baseline.log`

## 后续

按序 F[539_callback_codeval] → B[#npm_bind 诊断对齐 10 名]；182 是否入账待裁定；
build-exe 后续扩面另簇。本笔本地提交后请主会话代推并 PAT 核验，不停线。
