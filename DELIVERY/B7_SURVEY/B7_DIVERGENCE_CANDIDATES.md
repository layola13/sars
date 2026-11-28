# B7 起下一批次候选清单 — sars 对照 sci 全表面分叉普查

- 普查依据：muse 主会话派单 #19681（B6 冻结期间的新活，只普查与复现，不写修复、不碰 B6、不推送）。
- 基线：sars main=`df231d8`，工作树干净；sci 参考二进制 `/home/ctyun/sa_all/sci/zig-out/bin/sa`（仓库头 `500477e5`）。
- 语料：`/home/ctyun/sa_all/sci/demos/rosetta` 全部 968 例。
- 方法：三面实测。① verifier：`check` 全量 968 例双边跑，归一化 rc / trap_code / 报文。② 解释器：`run` 46 例代表性抽样。③ 发射面：`build-js`+node、`dump-c`/`build-c`+gcc 抽样编译运行。④ wasm：本机不可比（见"环境限制"）。
- 原始数据：`/tmp/b7_check_results.jsonl`（968 行）、`/tmp/b7_check_summary.txt`、`/tmp/b7_emit_results.jsonl`（46 行）、`/tmp/b7_emit_summary.txt`。复现件见本目录 `b7_*.sa`。

## 结论速览

| 面 | 结果 |
|---|---|
| verifier trap 码 / 名称 | 两边报错例 **完全一致**（code diff=0, trap name diff=0）——诊断码全面对齐 |
| verifier rc | **35 例**分叉：sars rc=0 通过 / sci rc=1 `MemoryLeak(E1012)` 参数寄存器出口未消费 |
| run（解释器） | **6 例**分叉：5 例 vtable/callback 间接调用 + 1 例 bitcast i64→ptr，sars 失败 / sci 成功 |
| js / c 发射面 | 行为一致（除受上游 check/import 前置影响者）；无独立发射分叉 |
| wasm | 本机不可比（sci 缺 `zig cc` 链接器） |

---

## B7-1 【高】解释器 vtable/callback 间接调用目标解析错误

- 现象：同一输入 `sars run` rc=1 `RuntimeTrap: invalid indirect call target <垃圾地址>`；`sci run` 正确调用目标并返回。JS/C 后端双边均正常 → 纯 `sa-interp` 缺陷。
- 复现件：`b7_1_vtable_indirect.sa`。实测：sars rc=1（invalid indirect call target 7022308338648768867），sci rc=7（正确返回 7）。
- 语料命中 5 例：`463_vtable_branchy_pick`、`485_vtable_param_float`、`638_vtable_sub2_code`、`769_callback_gt_code`、`878_vtable_multi_code`。
- 根因定位（仅诊断，未改）：`crates/sa-interp/src/exec.rs:273-289`，`CallIndirect` 把函数指针值当 `program.functions` 的数值索引（`target_value.as_u64() as usize`）。vtable `load t+0` 得到的是目标函数地址码（巨大数），非 0..N 索引 → 索引越界 RuntimeTrap。sci 用真实函数指针解析。
- 影响面：所有经 vtable / callback slot 的间接调用（解释器执行路径）；JS/C 后端不受影响。
- 建议编号：B7。

## B7-2 【高】解释器 bitcast i64↔ptr 全 64 位往返静默失败

- 现象：`sars run` rc=1、**无输出无错误**（静默失败）；`sci run` 正确返回。JS/C 后端双边均正常。
- 复现件：`b7_2_bitcast_i64_ptr.sa`。实测：sars rc=1 静默，sci rc=42（`ok` 分支正确）。
- 语料命中：`398_bitcast_i64_ptr`。
- 根因定位（仅诊断，未改）：`crates/sa-interp/src/value.rs:272` `OpKind::Bitcast`，`int→ptr` 走 `Value::Ptr(x as u64)`，`m=-1 as i64` 经 `mask` 后的 i128 表示在转 u64 时与 sci 全 64 位往返不一致；失败路径静默 rc=1 无诊断。
- 影响面：所有 bitcast i64↔ptr 承载非小整数 64 位模式的解释器执行。
- 建议编号：B8。

## B7-3 【中高】verifier 参数寄存器出口消费检查缺失（MemoryLeak 漏报）

- 现象：同一 .sa，`sars check` rc=0 输出 `OK: N functions, M instructions, gas used G`；`sci check` rc=1 `MemoryLeak(E1012): live registers remain at function exit`。
- 语料命中 **35 例**：`111_extern_c_abi`、`114_callback_from_c`、`115_opaque_pointers`、`133_select_macro_race`、`134_join_all_futures`、`135_async_streams`、`136_executor_task_queue`、`140_yield_now_suspend`、`161_generic_associated_types`、`166_specialization_fallback`、`170_marker_traits`、`177_unwrap_unwrap_err`、`182_mmap_memory_mapping`、`183_signal_handling_setup`、`185_dynamic_lib_dlopen`、`187_opengl_context_swap`、`18_option_map`、`196_lto_link_time_opt`、`198_control_flow_guard_cfi`、`19_result_question`、`245_contract_generic_monomorph_share`、`247_contract_semver_major_break`、`251_contract_resource_ownership`、`253_contract_callback_registration`、`255_contract_memory_allocator_swap`、`257_contract_log_facade`、`258_contract_thread_local_isolation`、`460_vtable_mem_spill`、`468_callback_slot_write`、`52_queue_rotate`、`530_vtable_codeval`、`538_vtable_pair_code`、`548_vtable_store_code`、`608_vtable_neg_code`、`658_vtable_load_code`（共 35 例，以 `/tmp/b7_check_results.jsonl` 实跑为准）。
- 根因定位（仅诊断，未改）：sci 在函数出口检查**参数寄存器**是否已消费（如 `111` 报 `@export add_pair` 的寄存器 `a`、`114` 报 `x`、`136` 报 `queue`），sars 对参数寄存器不做出口存活检查。典型源：`@export add_pair(a,b) { sum=add a,b; return sum }` 未 `!a`/`!b`。
- 与冻结项的关系：非 A3/E2 的 call-arg cap 精确消费（那批是 by-value 调用参数标记 escaped）；本项是**函数自身参数寄存器出口消费**，独立。验证：`sars check --legacy-state` 与默认一致，说明不是 legacy-state 口径差异。
- fail-before 复现件：`b7_3_param_leak.sa`（见本目录，sars check rc=0 / sci check rc=1 E1012）。
- 影响面：35/968 ≈ 3.6%，跨 extern/async/contract/vtable 多簇。
- 建议编号：B9。

## B7-4 【低】诊断报文与定位差异（trap 码一致）

- 现象：`243_contract_sig_mismatch_link`，两边都报 `CapabilityMismatch(E1013)`，但 sars 指向 `consumer/broken_consumer.sa:9` 且报文 `argument 1 of '@link_target' expects value capability, found borrow`；sci 指向 `main.sa:11` 的 `call @link_target(&slot)` 且报文 `call-site capability prefix does not match the callee contract`。
- 定性：诊断质量差异（定位文件/行、措辞），非漏报/误报。
- 建议编号：B10（可选，低优先）。

---

## 环境限制与剔除项（非 sars 缺陷，不计入候选）

- **wasm 发射面不可比**：sci `build-wasm` 需 `zig cc` 链接 wasm32-wasi，本机无该链接器 → `error[ExternalCompiler]: failed to launch zig cc ... FileNotFound`。sars `build-wasm` 正常。此项本机无法做双边行为对比，非 sars 缺陷。
- **sci check 超时 5 例**（`30_manual_guard_branch`、`32_trait_object_vector`、`334_indirect_import_shadow`、`335_ts_slice_inverted_bounds`、`35_iterator_fold`）：本机并发负载下 sci 进程 25s 超时（TimeoutExpired，见 `b7_check_results.jsonl` 的 5 条），属噪声，已从 rc 分叉中剔除。原稿误记为「6 例 / 含 `36_...`」，经核验实为 5 例（依 #19921② 更正）。
- **sci run 前置差异**：`02_mutability`（sci `InvalidAddress`）、`136_executor_task_queue`（sci MemoryLeak）等 `sci run rc=1 / sars rc=0` 例，其 js/c 后端双边一致，是 sci run 需要项目 ledger/更严消费策略的前置差异，不计为 sars 独立缺陷（136 的 check 面 MemoryLeak 已在 B7-3 计入）。
- **js/c build-fail 例**（`114_callback_from_c`、`245_contract_generic_monomorph_share`）：sci build-js/build-c 失败源于其 check/import 前置，非发射器行为差。

## 低优先跟进登记（#19921② 补记）

- **965_npm_handle_ctor_method c-face 分叉【低】**：`b7_emit_results.jsonl` 实测 `js_sars=js_sci=[0,"965\n"]`（JS 面一致），但 C 面 `c_sars=[0,"965\n"]` / `c_sci_build=0` / `c_sci=[1,""]`——sci C 发射编译成功但运行 rc=1 空输出，sars C 运行 rc=0 正常输出。check 面双边 rc=0、run 面双边 rc=1 空，故分叉仅在 C 发射执行阶段。按 #19921② 定低优先跟进，不并入 B10、本批不动。原稿未登记，此处补齐。

## 纪律声明

- 本轮**只做普查与复现**：未写任何修复、未碰 B6（285073d 与 /tmp/b6_hold/ 补丁）、A3/E2 冻结项未动、未推送远端。
- `.sa` 复现件放 DELIVERY/（入仓证据）；原始日志在 /tmp（不入仓）。
- 下一步：待 muse/主会话裁定优先级后，按 fail-before→实现→提交→直推→报核验逐批推进。
