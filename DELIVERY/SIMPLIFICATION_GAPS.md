# 《简化与缺失清单》 — sars 对照 sci 全表面

- 交付物：应答 10146 / 10183 / 10186 / 10187；禁止简化令下唯一交付。
- 对照基线：sars HEAD = `1a9a3c8`（`d531eee` B1 + `1a9a3c8` JS vtable 之上，origin/main `54d7a10`）；sci 现树 `/content/sa_all/sci`（SURFACE.md 为 fc638db5 时点快照，引用时按时点理解）。
- 口径声明：本清单只列"简化/缺失/粗粒度/诊断缺失"，不把任何按语料名集定义的"打平"当作 parity（10146 已暂停该口径）。每项含 sars 现状（file:line）、sci 对照（file:line）、分类、影响。所有 file:line 来自逐行源码核对，未推测。
- 分类：**[简化模型]** 用更弱/更粗的模型冒充语义；**[能力缺失]** sci 有而 sars 无对应实现路径；**[粗粒度]** 有实现但语义精度不足；**[诊断缺失]** 错误被 0/noop/warning 掩盖或报错信息不可用。
- 本清单期间未改任何实现代码、未开新簇、未提交；R1 修复分叉 A/B/C 保持 open 登记（10187），未动手。

---

## 0. 显式声明的已知差距（原文引用，非逐项分析）

- sars README `/content/oc-box-1-work/README.md:50-58`：
  > "Known gaps: interpreter/JS semantics for a few macro-heavy programs (e.g. `104_if_let_chains` takes its error branch, `81_kv_store` hits an out-of-bounds read under `run`); the interpreter models fallible values as plain values, so `?` on a null pointer propagates early (demo 372 runs its error branch); JS run coverage for struct/vtable-heavy and async demos; native/LLVM and C backends are not ported. Fallible calls now carry an explicit status in the interpreter (`?` unpacks success, including a null payload), and `#def` offsets inside address operands are expanded after macro flattening."
  注：同段"按普通值建模"与"现已显式状态"自相矛盾，属历史叠加，未清理。
- sars README `:34-42`：966/970 `sa check`（余 243/253/257/335 上游亦失败）；913/970 `build-js`（Node 18）；806/970 `run`。
- sci `docs/completeness_evaluation_2026-06-25.md:11-13`：核心 75–80% 完成度；"自给自足工具链"过誉——verifier+IR 自包含，但每条机器码/wasm 路径是 `zig cc` 前端。
- sci 同文 `:45,52-54,67-84`：layout 仅基本类型 struct（无 enum/union/嵌套）；def-dict 算术无除/取模；`emit_wasm/` ~1400 行为死代码，实际 `build-wasm` 走 LLVM + `zig cc -target wasm32-wasi`；无泛型/惰性迭代器/trait，`Box/Rc/Arc/Cell` 仅 in-place u64，无 u128/i128。
- sci `STATUS.md:34-44`：`zig build test` 477/498，20 命名失败 + 26 unit 文件失败，多数为既存/环境类；`c-matrix` 因 `sa_install_sigpipe_ignore` 未定义（既存 emit_c 缺口）。
- sci `DELIVERY.md:21-35,57-61,156-166`：http_server 46 中 33 MISSING、tls_server 18/18、ws_client 12/12、dtls 11/11、http2 14/14、quic 29/29、ssh 26 EXPLICIT；http_client send 族为 echo 桩，零真实网络调用；未补面含 TCP listen/accept、unix 全族、向量化读写、句柄克隆、ja3/ja4、ws 帧、url_parse 等。
- sci `FINDINGS.md:57-65`：caching_sha2 full auth、TLS 传输、LOAD DATA LOCAL INFILE、多语句/多结果集、游标、连接池/Unix socket、流式 fetch 等全部显式拒绝。
- sci `docs/agent_daemon_protocol.md:114,145`：`--daemon` flag 未按文档实现（现为 `SA_DAEMON_SOCKET` 环境变量）；SA-DAEMON-001…005 码在 `src/` 零命中。
- sci `docs/agent_native_compiler_gap_and_plan.md:42`：`sa contract freeze / sa func verify / sa func emit / sa coordinate / sa link --contract` 均不存在于 `Command` 枚举；`:46-49`：多 agent 承受力"几乎从零起步"。
- sci `docs/sab_distributed_compilation.md:7,42-46`：`build --emit-sab`、SAB 链接器、跨模块 Referee、模块缓存失效、调试信息保留未实现。

---

## 1. 程序表面：Verifier / Referee（sa-verify，sa-ir）

| # | 项 | sars 现状 | sci 对照 | 分类 |
|---|---|---|---|---|
| A1 | 寄存器状态模型 | `RegState`/`RegKind` 为裸 enum，`FnState` 用并行 HashMap（regs/fallible/borrow_origin/origin_base/escaped/borrow_params），`state.rs:3-33`；无复合态 | `CapabilityMask` 为 12 位 u16（active/locked_read/mut/consumed/borrow_view/ffi_borrow/untracked/fallible/immutable/interior_ptr/owns_children），`common/capability.zig:3-19`；`maskName` `:29-45` | [简化模型] 高：下游多数缺口根因 |
| A2 | Phi/合并格 | `FnState::merge` 仅合并 regs，`fallible`/`borrow_origin`/`origin_base` 求交、`escaped` 求并；冲突只发 warning `phi_warning`（`state.rs:53-116`，`lib.rs:104-129,137-184`） | label 快照捕获 state+origins+locks+flags+interior tree+offsets+alloc sizes（`const_trap.zig:352-400`），进入 label 时恢复（`clear_borrow_ctx.zig:180-212`）；分支点比较快照并 trap（`verify_body_arms_assign_branch.zig:154-260`），带显式 PATCH 的 join lattice（`const_trap.zig:304-342,463-504`） | [简化模型][粗粒度][诊断缺失] 高：不健全地接受冲突路径 |
| A3 | `escaped` 在 join 处求并 | `out.escaped = self.escaped.union(&other.escaped)`（`state.rs:89`），`check_leaks` 跳过 escaped（`rules.rs:559-562`） | 每函数精确预计算 per-slot `consumed_in_function`（`snapshot_merge_state_in_scope.zig:60-152`），exit 扫用精确集（`clear_borrow_ctx.zig:378`） | [粗粒度] 高：单分支逃逸即全路径免泄漏检查 |
| A4 | 重定义/liveness 精度 | `check_redefine` 豁免 `Owned{Scalar}`（`rules.rs:44-46`），`define` 只抑制自更新（`rules.rs:75-84`），无 const 检查 | `assignValueCtx` 拒绝任何 live reg（除 consumed/untracked/ephemeral/raw）（`collect_metadata.zig:527-544`）；`writeCheck` 拒绝 immutable 写（`477-514`） | [能力缺失][简化模型] 中高 |
| A5 | 分支条件标记 | `Br`/`BrNull` 落 default 分支（`rules.rs:703-708`），仅 `read_operand` | `markBranchCondition`（`clear_borrow_ctx.zig:176-178`），由 `armBr/armBrNull` 调用；分支临时量免 exit leak（`:376`） | [能力缺失] 低中 |
| A6 | CFG 终止检查 | fallthrough 仅 warning 且仅 `index+1<len`（`cfg.rs:82-99`），末块无终止符不报，`find()` 取首终止符忽略后续指令（`cfg.rs:59-61`） | 函数体末无终止符为 error（`verify_body.zig:163-165`）；块未终止/终止后有指令为 error（`:211-214,389-392`） | [诊断缺失][能力缺失] 中 |
| B1 | capability 结构空壳 | `Capability` 定义 `FALLIBLE`（`capability.rs:11`），`from_prefix` 仅映射 4 前缀（`:17-24`），verifier 仅用于文案（`rules.rs:472-475`） | mask 位驱动全部检查（`common/capability.zig:3-19`），真值表 `:74-100` | [能力缺失] 中 |
| B2 | FFI airlock | `RawCast/AssumeSafe/AssumeBorrow` 落 default（`rules.rs:703-708`）；`Trap::IllegalUnsafeContext` 定义但 verifier 从不发 | `armRawCast/armAssumeSafe/armAssumeBorrow` 在 `@ffi_wrapper` 外 trap（`verify_body_arms_assign_branch.zig:104-130`）；FFI borrow 态（`collect_metadata.zig:617-630`） | [能力缺失] 高：安全边界 |
| B3 | `@const` 数据不可变性 | 仅拒绝对 const 的 mutable borrow（`rules.rs:224-236`），const 从不 seed 进 state，move/release/write/return 全不检 | `seedConstSymbols` 写 `active|immutable`+flag（`const_trap.zig:44-55`），写/消费/release/borrow/call/return/transfer 各有 guard（多处） | [能力缺失] 高 |
| B4 | `default_immutable` 模式 | `VerifyOptions` 仅 gas/leaks/caps/phi（`lib.rs:14-20`） | `VerifyOptions.default_immutable`（`reg_state_change.zig:83-94`），`allocDefaultMask`（`:610-616`） | [能力缺失] 中 |
| B5 | `transfer` 指令 | `InstructionKind` 无 `Transfer`（`instr.rs:478-513`），Trap 中相关项空壳 | `armTransfer` 校验目标是 owned root、非 immutable/consumed/active borrow，置 `owns_children`（`verify_body_arms_alloc_borrow.zig:251-302`） | [能力缺失] 中 |
| C1 | ephemeral scalar 折叠 | `Op/Load/Take` 一律定义 `Scalar`（`rules.rs:612-617`），`Assign` 用 `is_scalar_operand`（`:179-187,606-611`），Scalar 免重定义、release 为 no-op（`:46,309-311`） | 指针 load 得 raw pointer flag、非指针 load 得 ephemeral scalar、take from borrow 建 interior tree（`verify_body_arms_alloc_borrow.zig:61-80`），atomic/op/assign 传播 flag，exit 免 leak（`clear_borrow_ctx.zig:359`） | [粗粒度][简化模型] 高：装入指针与标量不可分 |
| C2 | scalar release 跳过 use-after-move | 早退不做任何状态检查（`rules.rs:309-311`） | raw/ephemeral no-op 路径仍调 `readCheck`（`assign_branch.zig:64-83`） | [能力缺失] 中 |
| D1 | fallible 状态模型 | `FnState.fallible: HashMap<String,bool>`（`state.rs:28`），call 设置、Move 复制（`rules.rs:649-655,621-625`）；`Try` 仅显式 `false` 才报错（`:684-701`），`Return` 无 fallible 检查（`:663-677`） | `.fallible` mask 位（`common/capability.zig:12`），`armTry` 无条件检查 `(src&fallible)==0`（`call_return.zig:243-246`），`armReturn` 拒绝非 fallible 函数返回 fallible（`:290-293,327-330`） | [简化模型][能力缺失] 高 |
| D2 | EarlyReturnLeak | `EarlyReturn` 落 default，leak 检查只在 `Return` 调用（`rules.rs:676`），Trap 空壳 | `early_return` 是 exec+terminator（`const_trap.zig:64-75`），`armTry` 扫所有 live reg 并 trap（`call_return.zig:248-274`） | [能力缺失] 高：错误路径泄漏 |
| E1 | interior pointer / 字段级 borrow | `FnState` 无 interior 数组（`state.rs:25-33`），`PtrAdd` 落 default，`Take` 仅读并定义 Scalar | `InteriorContext` 数组（`reg_state_change.zig:551-561`），tree attach/detach/consume（`snapshot_merge_state_in_scope.zig:154-211,416-467`），`ptr_add` 字段身份与 OOB（`verify_body_arms_alloc_borrow.zig:182-210`），重叠字段 borrow 检查（`:394-414`） | [能力缺失] 高 |
| E2 | escape 标记粗粒度 | 任何 call arg、任何 store addr/value 都 `mark_escaped_alloc`（`rules.rs:392-398,400-401,631-632`），不看 `&`/`^`/by-value cap | 精确 move 消费（`call_return.zig:134-156`），native 契约边界消费+const-borrow 豁免（`clear_borrow_ctx.zig:123-174,364-375`） | [粗粒度] 高：假阴+假阳 |
| E3 | stack escape | `RegKind::StackAlloc` 创建（`rules.rs:599-604`）但无 `StackEscape`，leak 扫只考虑 Alloc/Param（`:556-578`） | move/release/return/native 边界拒 stack alloc（`collect_metadata.zig:582-584`，`assign_branch.zig:91-93`，`call_return.zig:139-141,303-305`） | [能力缺失] 高 |
| E4 | store/load/ptr_add 写检查与静态边界 | `Store` 仅读操作数+mark escaped+定义 Value（`rules.rs:627-634`），`Load` 读+定义 Scalar（`:612-617`） | `writeCheck`（`collect_metadata.zig:477-514`）、`checkStaticOffsetInBounds`（`snapshot_merge_state_in_scope.zig:334-366`） | [能力缺失] 高：越界/immutable 写被接受 |
| E5 | native escape 消费 | `Native` 落 default（`rules.rs:703-708`），`Instruction::reads()` 对 Native 返空（`instr.rs:619-625`） | `armNative` 消费所有引用 reg 并拒 stack escape（`assign_branch.zig:141-152`） | [能力缺失] 中 |
| F1 | import 环诊断 | 在 `sa-flatten`（非 sa-verify）：`Context.visited` + 规范路径栈，泛化文案 `ForbiddenSyntax("import cycle detected during flattening")`（`sa-flatten/src/lib.rs:33-34,98-108`），visited 短路重复（`:110-112`） | `CyclePolicy`（strict 默认，`module_loader/cycle.zig:78-94`），`detectCycle` 返回 `CycleRecord{cycle_path,entry}`（`:152-177`），`checkCycle`→`error.ImportCycle`（`:195-207`），路径化报告（`:236-308`） | [诊断缺失][粗粒度] 中 |
| G1 | 诊断结构 | `Diagnostic` 有 `register` 字段（`trap.rs:224-232`）但 Display 只打 loc/code/message/function/hint（`:266-276`）；phi 用 `{:?}` 打 enum（`lib.rs:118-119`）；未知跳标签复用 `UnknownRegister`（`cfg.rs:70-77`） | `TrapReport` 带 register(s)/expected/actual mask **及名字**/source/original text/upstream loc/context/function/repair（`common/trap.zig:70-109`），路径感知 phi hint（`snapshot_merge_state_in_scope.zig:543-617`） | [诊断缺失] 中高 |
| G2 | 错误去重 | `DiagSink::key=(trap,message,loc.line)`（`lib.rs:47-64`）吞掉同行不同诊断 | fail-fast `union(ok,trap)` + 显式 trap 优先级（`reg_state_change.zig:72-75`，`verify_body.zig:122-129,179-183`） | [粗粒度][诊断缺失] 中 |
| G3 | trap 目录空壳 | verifier 实发 13 类 trap；`Trap` enum 仍声明 `VTableSignatureMismatch/StackEscape/ConstMutation/InteriorPtrEscape/EarlyReturnLeak/IllegalUnsafeContext/ReadWriteConflict/GpuSubsetViolation` 等从不发（`trap.rs:5-72`） | 对应 trap 在 sci 全部 live（`common/trap.zig:5-68`） | [能力缺失] 高：G 段总览 |
| H1 | 增量验证 | 每次全量 `verify`（`lib.rs:187-236`），全仓无 cache/hash；CLI 仅在 compile 中调 verify（`sa-cli/src/pipeline.rs:110-111`） | SHA-256 verdict cache（mode 折进 digest，`incr_verify.zig:14-18,41-123`），接入 check/compile（`cli/check.zig:101-121`，`cli/compile_core.zig:95-96,197-223`） | [能力缺失] 中 |
| I1 | 原子序模型 | `check_atomic` 只比较 `previous != order`（`rules.rs:372-390`），仅 `AtomicRmw` 调用且 key 丢 offset（`:678-680`），`Cmpxchg/AtomicLoad/AtomicStore/Fence` 落 default | `atomicKey=(reg<<32)|offset` + ordering 位（`const_trap.zig:210-234`），`sameAddressRmwCompatible`（`:236-255`），cmpxchg/rmw 都调（`alloc_borrow.zig:129,142`） | [粗粒度][能力缺失] 中高 |
| I2 | gas 模型 | `gas_total` 静态求和所有指令 gas_cost（`lib.rs:67-72`，`instr.rs:630-642`），一次比较用首函数 loc（`:189-192,74-86`），worklist 10 万步静默 break（`:145-148`） | `gas_steps/gas_alloc_bytes/has_unbounded_loop/call_depth`（`verify_body_ctx.zig:44-47`），回边标 unbounded（`assign_branch.zig:157,220,257`），`GasReport` bounded/unbounded（`verify_body.zig:195-204`） | [简化模型] 中 |
| I3 | 元数据/签名损坏校验 | `initial_state` 直接索引 sig.params/locals，无校验（`lib.rs:88-102`） | 校验 `param_ids.len>=params.len` 与 scope slot，发 `corrupted_signature`（`verify_body.zig:321-354`，`collect_metadata.zig:409-442`） | [能力缺失] 中：畸形 SAB 可 panic |
| I4 | 并行验证/阶段报告 | 单 worklist（`lib.rs:137-184`） | 自适应 worker 数、stream merge、trap 优先级（`free_annotated_slice.zig:54-129,197-415`），`VerifyStageReporter`（`reg_state_change.zig:96-103`） | [能力缺失] 低（性能/可观测） |
| I5 | 领域钩子 | `FunctionKind::Gpu`、`PackageGrant*`、`Sax*` trap 在 enum 中存在（`types.rs:176-184`，`trap.rs:5-72`）但 verifier 不发 | `checkGpuSubset`（`free_annotated_slice.zig:417-512`），SAX hooks（`alloc_borrow.zig:83-88` 等），package grants/`@sys_*`（`call_return.zig:51-76`） | [能力缺失] 中 |

---

## 2. 发射面：JS（`sa-emit-js`）与 WASM（`sa-emit-wasm`）

### W0 前提：sci 的 WASM 后端形态

sci 的 `src/emit_wasm/` 只有编码工具箱（mod/encoder/sections/opcodes，`CJ/emit_wasm/mod.zig:10-13`），`rg emit_wasm` 在 `src/` 无调用方；生产 `build-wasm` 走 LLVM IR + `zig cc -target wasm32-wasi`（`src/cli/build_wasm.zig:81-95,124-138`，`src/driver/zigcc_wasm.zig:31-32`）。故下表凡 sci 仅 toolkit 的项，"参考语义"来自 LLVM/WASI 产物或 toolkit 显式能力声明，而非同形态手写发射器。sars 是完整手写直接模块发射器（sa-emit-wasm/src/lib.rs:120-245）。

### 2.1 JS 发射器

| # | 项 | sars 现状 | sci 对照 | 分类 |
|---|---|---|---|---|
| J1 | `.d.ts` 发射整体缺失 | 模块列表无 dts（`lib.rs:1-8`），`JsOptions` 仅 format/debug/entry/memory_pages（`:16-22`），CLI 只写一个 JS 文件（`commands.rs:160-174`） | `emitDtsToString/dtsPathForJsPath/emitDtsToFile`（`emit_js.zig:261-285`），`emit_js/dts.zig:18-97`，`entry.zig:378-475`，`DTS_EMIT_DESIGN.md:9-27` | [能力缺失] 高：TS 消费者零支持 |
| J2 | 用户函数边界无类型编组 | 形参直接发射（`lower.rs:187-193`），返回值直接 `return {expr}`（`:248-255`），无 PrimType 分派 | `SURFACE.md:43-56` 实测边界表，`DTS_EMIT_DESIGN.md:29-47` 裁定 i64/u64/ptr/handle=`number|bigint` | [简化模型] 高：双表示无人声明/保证 |
| J3 | void 返回 0 vs undefined | 非 fallible 无值返回 `return;`（`lower.rs:344-350,354-366`），fallible 发 `{v:0,ok:true}` | `emit_js/dispatch.zig:55-56` 明确 `return 0;`，`SURFACE.md:52` 实测 `noop()`→number 0；wrapper 才 undefined（npm.zig:377-381） | [简化模型] 中：与 sci 实测不符 |
| J4 | fallible 对象形状与 `?` 传播 | 成功 `{v:X,ok:true}`（`lower.rs:248-254,345-347`），`Try` 只 `sa_unwrap`（`:434-437`），失败即 `sa_panic`（`runtime.rs:122-128`），`EarlyReturn` 也返回 ok:true、不传播 callee 失败对象（`:354-366`） | `{s,v}`（`dispatch.zig:45-62`），`try_/early_return` 用 `__sa_fok` 检查并原样 `return __t`（`:72-87`），helper（runtime_header_memory.zig:90-91），`SURFACE.md:53` | [能力缺失][简化模型] 高：跨函数 `T!` 不兼容 |
| J5 | 64 位 number/bigint 运算 | `sa_add/sub` 只要出现 bigint 就 `BigInt.asIntN(64)`（`runtime.rs:140-147`），32 域 `|0`，`sa_mul` 用 `Math.imul`（`:148-151`），比较返 JS boolean（`:224-263`），字面量阈值 i32（`ident.rs:87-93`） | helper 仅在操作数越 32 域才提升 BigInt 且 add/sub 保 double 精度（`runtime_header_memory.zig:195-219`），imm_i64/u64 按 32 位边界选字面量（`instr.zig:81-108`），比较返 1/0（`:210-215`） | [粗粒度] 中高：>2^31 的 32 位语义被符号截断 |
| J6 | i1 边界与窄整型转换 | `ty_code(I1)="i1"`（`lower.rs:637-655`），load i1 读字节（`runtime.rs:77`）、store i1 仅 `&1`（`:95`），`sa_cvt` 单函数按参分派（`:318-383`），zext srcbits 靠类型表启发式（`lower.rs:583-597`） | i1/i8 共用 load/store i8（`ident.zig:149-177`），每目标宽度专用 cvt/srcmask（`runtime_header_memory.zig:261-381`），宽度静态跟踪（`widths.zig:23-38`，`instr.zig:181-194`） | [粗粒度] 中高：启发式位宽可能取错源位 |
| J7 | v128  invented representation | `ty_code(V128)` 与 `sa_v128/*` 以 `[bigint,bigint]` 建模（`lower.rs:653,612-617`，`runtime.rs:384-409`） | `SURFACE.md:54` 明确"无表示、任意 JS 值透传"；`dts.zig:37,64-77` 对导出面 v128 报 `UnsupportedDtsType` | [简化模型][能力缺失] 高：sci 拒绝的形状 sars 静默产出 |
| J8 | 字符串编组地址归一/边界 | wrapper 识别相邻 `(ptr,len)` 调 `sa_utf8/sa_bytes_view`（`npm.rs:121-137`，`runtime.rs:494-501` 用 `sa_num` 不归一/不检界），string-out `String(...)` 强制转换（`npm.rs:177-193`） | `__sa_npm_args` 用 `__sa_addr`（`npm.zig:277-321`），`__sa_jsh_str_out` 要求 `typeof v==="string"` 并写 out_len/检界（`runtime_handle.zig:36-53`） | [简化模型][诊断缺失] 中高 |
| J9 | npm bind 解析无校验、导入不分组 | `collect()` 仅 `split_once(':')`/`split_once('.')`（`npm.rs:14-38`），无 spec/export/冲突校验，每 bind 单独 import（`:40-75`） | 完整校验（`npm.zig:90-180`），按 specifier 合并 named import（`entry.zig:518-559`） | [诊断缺失][粗粒度] 中 |
| J10 | npm wrapper ESM/CJS 不对称丢失 | wrapper 只写 `SA_EXTERNS`（`npm.rs:172-199`，`emit.rs:169-171`），导出列表仅 functions，ESM/CJS 完全对称（`emit.rs:138-197`） | S2 wrapper 在 ESM 是 `export function`、CJS 不导出（`npm.zig:260-262`，`entry.zig:329-342`，实测 `SURFACE.md:14-29`） | [能力缺失][简化模型] 中：ESM 无法按名导入 wrapper |
| J11 | 运行时助手集合与内存管理缩减 | 仅 `U8/I32/U32/F32/F64/DV`（`runtime.rs:1-18`）、单向 bump alloc（`:50-61`）、`sa_release` 恒等（`:129-131`）、`Release` 发空（`lower.rs:375`）；无 free/frame/slab | 固定 8 助手（`SURFACE.md:85-97`），`__sa_alloc/free/salloc/sfree/mem_used`、slab/free-list/live Set、零页 65536（`runtime_header_memory.zig:19-164`），函数体 fstack push/finally pop（`funcs.zig:193-234`） | [能力缺失][简化模型] 中高：无回收/栈槽生命周期 |
| J12 | 函数指针/vtable 调用约定 | vtable 槽写 `SA_FN_BASE+index`（`emit.rs:76-88`），无 `__sa_ftable`，`call()` 按 typeof 四路分派（`runtime.rs:436-447`），`CallIndirect` 复用（lowering `:331-340`） | 显式 `__sa_ftable`（`entry.zig:55-67,158-164`）、`__sa_fnptr/fnval/call_indirect`（`runtime_header_memory.zig:86-89`），间接走表（`instr.zig:585-635`） | [粗粒度][简化模型] 中：错误目标诊断弱 |
| J13 | thenable/非数值返回无诊断 | wrapper 直接 return 或 handle_new（`npm.rs:194-198`），无 Promise/非数值检查 | 三路径均有 thenable trap 与非数值 trap（`npm.zig:370-386`），undefined 句柄拒绝（`runtime_handle.zig:9-15`） | [诊断缺失] 中：异步 npm 绑定被静默当句柄 |
| J14 | 回调队列与句柄代际 | `SA_CB_QUEUE` + make_callback/poll（`runtime.rs:455-479`），句柄即数组下标（`:480-493`），release 仅置 null，poll 返 BigInt 0n/1n | `gen*2^32+(idx+1)` BigInt，stale/released trap（`runtime_handle.zig:6-35`），退出未排空告警（`:54-85`），poll 返 number（`:71-79`） | [简化模型][诊断缺失] 中：回收槽位被陈旧句柄别名 |
| J15 | 常量发射方式 | 每 const 运行期 `sa_alloc`+逐字节写（`emit.rs:60-110,122-128`），vtable fallback 索引 0（`:79-86`），Null/Undef→0（`:75`） | 静态地址 4096 起、bump_start 65536、空 const 保留 8 字节、vtable `0x5341...+fidx` 小端（`entry.zig:165-220`），const 槽参与寄存器初始化（`funcs.zig:200-232`） | [简化模型] 中：地址不稳、无零页 |
| J16 | 原子操作语义降级 | `Atomics.load/store/compareExchange` 固定 I32 + `sa_atomic_index`（`lower.rs:400-424`，`runtime.rs:412-433`），忽略 order 与元素类型；`Fence` 空（`:433`） | 明确单线程降级 load/ok/store 与 load/op/store（`dispatch.zig:242-294`），同样忽略 ordering，但按 memPrimType 选 helper（`ident.zig:140-177`） | [粗粒度] 中：64 位原子与内存序不正确 |
| J17 | 寄存器模型 | `r_<js_ident>` 命名寄存器，类型由 ins.ty 或首个同型 read 推断（`lower.rs:60-71,135-178,211-222`），Move 清零源（`:376-387`） | 数字槽 r0..rn、const 槽、参数绑定（`funcs.zig:185-234`），owned-base provenance 决定 release 是否真 free（`dispatch.zig:348-362`，`prov.zig`），栈槽提升（`funcs.zig:89-102`） | [粗粒度][能力缺失] 中高：无所有权分析，release 全 noop |
| J18 | 函数名净化多 `sa_fn_` 前缀 | `js_func_name` 对 `call/sa_*/memory/SA_FUNCS` 加 `sa_fn_`（`ident.rs:47-61`） | `jsFuncName` 仅去引号/`@` 后走 `jsIdent`（`emit_js/ident.zig:37-68`） | [简化模型] 低中：导出名与调用名不一致 |
| J19 | debug/source 形态 | `sa_trace` 空函数（`runtime.rs:132-133`），每 block 调用（`lower.rs:206-208`），无 source 头 | `// sa-sig:`（`funcs.zig:197-199`）、`// source:`（`entry.zig:157`），debug_comments 默认 true（`types.zig:19-31`） | [能力缺失] 低 |
| J20 | 未绑定 extern 存根 | `SA_EXTERNS[name]=function(){return 0;}`（`emit.rs:173-179`） | `__sa_trap("extern not linked")`（`funcs.zig:178-183`），14 类内建 shim（`:151-166`），extern stub DCE（`entry.zig:274-327`） | [诊断缺失][能力缺失] 高：漏链接返回 0 而非失败 |
| J21 | 变参 | `VaStart/VaArg/VaEnd` 发 SA_VA/SA_VARARGS 队列（`lower.rs:453-462`，`runtime.rs:16-17,436-445`） | 签名层直接 `UnsupportedVariadic`（`entry.zig:47-51`），width 层丢弃（`widths.zig:157-163`），linear emitter 无分支→trap（`dispatch.zig:377-381`） | [简化模型] 中：接受/拒绝方向相反 |
| J22 | native 指令 | 整条拒为 `UnsupportedInstruction`（`lower.rs:463-470`，`tests.rs:200-207`） | 空 asm sideeffect no-op、非空转义后 trap（`dispatch.zig:363-376`） | [简化模型] 中：sci 可编译模式在 sars 构建失败 |
| J23 | 分支真值/Null 判定 | `Br` 用 `({c})?t:e`（`lower.rs:305-311`），`BrNull` 用 `({c})==0`（`:312-321`） | `__sa_truthy(v)`（`dispatch.zig:391-397`），br_null 严格 `===0||===0n`、NaN 非 null（`:398-410`） | [粗粒度] 中：`==` 对非数值值判定不同 |
| J24 | 入口 runner | `process.exit(Number(code)|0)`，ESM 守卫仅 `.mjs`（`emit.rs:200-213`） | ESM/CJS 均直接 main()+try/catch，扩展名正则 `\.(mjs|js|cjs)$`（`entry.zig:338-341`） | [简化模型] 中：返回值当退出码、`.js` ESM 不自动执行 |
| J25 | 默认内存页数 | crate 默认 2 页（`lib.rs:24-32`），CLI 显式覆盖 256（`commands.rs:160-165`） | 选项默认 256 页（`types.zig:19-31`），无第二默认值 | [简化模型] 低 |

### 2.2 WASM 发射器

| # | 项 | sars 现状 | sci 对照 | 分类 |
|---|---|---|---|---|
| W1 | **回边控制流直接 UNREACHABLE** | `branch_depth` 对 `j<=current` 返 None（`lower.rs:327-339`，测试明确断言回边 Some(0)→None，`:968-975`），`emit_branch` 在 None 分支发 DROP+UNREACHABLE（`:341-361`） | `controlOpcode` jmp→br、br→br_if（`emit_wasm/opcodes.zig:185-195`）；生产 LLVM 支持正常循环（`cli/build_wasm.zig:124-138`） | [能力缺失] **致命**：所有回边运行时 trap，任何循环不可执行（R1 实证条目，见 §8） |
| W2 | 块/循环结构与返回收尾 | 每 block 一个 BLOCK、最外层套一个 LOOP（`lower.rs:913-945`），Return 发 RETURN+UNREACHABLE（`:835-843`），非 void 尾补 0（`:946-948`），无 br_table/select/loop br | toolkit 以 std.wasm.Opcode 全量枚举为准（`opcodes.zig:6-10`），FunctionBody 保留原始 opcode 字节（`sections.zig:78-83`） | [简化模型] 高：仅前向 DAG，多余尾值/UNREACHABLE |
| W3 | funcref 表与元素段类型不安全 | 只要出现 CallIndirect 就把所有函数（含 import）按全局索引填单表（`lib.rs:143-154,227-234`，`sections.rs:78-99`），间接调用 SA_FN_BASE 减法+I32_WRAP_I64（lowering `:809-826`），返回类型靠 vtable 签名猜（`:126-160,780-834`） | SectionId 含 table/element/data_count/start（`sections.zig:18-33`），ImportDesc 支持 function/memory/global（`:55-65`），类型映射由 MapOptions/NumericDescriptor/MemoryDescriptor 驱动（`opcodes.zig:55-102`） | [粗粒度] 中高：不同签名函数可被调用，type index 靠猜 |
| W4 | 内存模型 | memory_section 只写 min pages、flag=0（`sections.rs:70-76`），entry 把 heap_start 存地址 0（lowering `:903-908`），Alloc/StackAlloc 同一 bump 无 free（`:631-656`），const 从 1024 起（`lib.rs:48-79`） | `writeMemoryType` 校验 shared 须有 max 并写 flags/min/max（`sections.zig:483-497`），alloc 映射 malloc import（`opcodes.zig:175-183`），生产链 wasm32-wasi+sa_std（build_wasm.zig:132-138） | [简化模型][能力缺失] 中高：无释放/重分配、常量与堆可冲突、无 shared/max |
| W5 | WASI 导入与 shim | 所有 extern 一律 `(env, display_name, func)`（`lib.rs:156-159`，`sections.rs:22-29`），无 wasi_snapshot_preview1/fd_write/proc_exit，无 libc/std 链接 | WasiImport 6 调用、`wasi_module_name="wasi_snapshot_preview1"` 与名称表（`opcodes.zig:36-43,107,145-153`），链接 wasm32-wasi（driver/zigcc_wasm.zig:31-32） | [能力缺失] 高：产物不按 WASI 约定跑标准 IO/进程 |
| W6 | 浮点：只有 f64 算术 | storage_type 把 F32/F64 都映射 VALTYPE_F64（`opcode.rs:147-153`），numeric_opcode 只发 F64_*（lowering `:368-482`），f32 常量 helper 存在但 `#[allow(dead_code)]`（opcode.rs:1,115-131），load 用 F64_PROMOTE_F32、store 用 F32_DEMOTE_F64（lowering `:684,708-717`） | `wasmValueTypeForPrim` 保留 .f32（`opcodes.zig:163-173`），算术/比较有 f32 分支（`:315-325,480-504`），load/store 有 .f32_*（`:276-313`），f32.const 4 字节编码（`sections.zig:514-520`） | [粗粒度][能力缺失] 中高：f32 ABI/舍入精度不同 |
| W7 | 转换指令 zext/sext/bitcast | Trunc/Zext/Sext/Bitcast/Fpext/Fptrunc 返回 None（lowering `:470-471`），只映射 Fptosi/Sitofp/Uitofp（`:467-469`），convert() 只覆盖 4 对、其余静默 `_=>{}`（`:241-252`） | 9 种转换同为 UnsupportedInstruction，但 neg/not 给 preload 描述符（`opcodes.zig:437-452`），显式 MapError 分类（`:45-53`），InitExpression 支持各 valtype（sections.zig:504-535） | [简化模型][诊断缺失] 中高：静默吞掉未覆盖对、产生不合法栈类型 |
| W8 | i1/窄整型 load-store | load_opcode(I1)=I32_LOAD8_U、store I1/I8/U8=I32_STORE8（`opcode.rs:162-197`），store 前不按类型归一（lowering `:702-722`），align 硬编码 | 同 opcode 映射但按 PrimType 算自然对齐（`opcodes.zig:276-313,231-244`） | [粗粒度] 中 |
| W9 | 段编码覆盖 | 固定发 0/1/2/3/4/5/7/9/10/11（`lib.rs:219-244`），无 global(6)/start(8)/data_count(12)，data 段恒 active/mem0（sections.rs:115-126），name 段只含函数名（`:128-141`） | SectionId 覆盖 0..12（`sections.zig:18-33`），GlobalSection/passive DataSegment/memory_index（`:55-94,251-285,354-392`），write() 固定顺序（`:430-444`） | [能力缺失] 中：无法表达 global/passive/多内存/起始函数 |
| W10 | 验证与诊断保证 | 诊断累积后统一返回（`lib.rs:215-217`），未知寄存器/符号仍发射 0（lowering `:271-294`），回边静默 UNREACHABLE，无结构校验 | ModuleAssembly.validate() 校验函数体数/内存限制/u32FromLen 溢出（`sections.zig:453-457,483-486,575-577`），MapError 分类完整（opcodes.zig:45-53,528-559） | [诊断缺失] 中高：可输出"看似成功但语义错误"模块 |
| W11 | 多返回值/fallible 调用 | functype 支持多结果数组（sections.rs:3-11）但 signature_type 最多压一个结果（`lib.rs:82-109`），fallible 调用直接诊断拒绝（lowering `:759-767`，test lib.rs:410-426） | FunctionType.results 为 slice 并按长度写 count（`sections.zig:40-43,471-481`） | [能力缺失] 中：`T!` 多值 ABI 在 wasm 不可用（JS 后端却用对象模拟） |
| W12 | 导出面与入口 | 只导出一函数（entry→首个 Exported→首函数）与 memory（`lib.rs:189-213`），CLI 明确忽略 `--entry`（commands.rs:179-185） | Export 列表化写入（sections.rs:67-71,287-322），生产由 wasm-ld 决定导出 | [简化模型][能力缺失] 中：无法多函数/表/全局导出，entry 无效 |
| W13 | 参数/返回 ABI 折叠 | 所有整数 I64、F32/F64→F64、Ptr/BlobHandle→I64（`opcode.rs:133-153`），无 wasm32 目标概念 | Target.wasm32/wasm64 决定 ptr 为 i32/i64，wasmValueTypeForPrim 保留 8/16/32/64 位与 v128（`opcodes.zig:12-15,156-173`） | [粗粒度] 中高：与外部 wasm32 模块/宿主签名必不一致 |
| W14 | 常量布局覆盖 | const_layout 仅 VTable 与 bytes() 非空值（`lib.rs:48-79`），bytes() 只认 Bytes/Utf8（sa-ir/types.rs:385-391），Int/Float/Bool/Aggregate 被 continue 丢弃 | constBytesLen/fillConstBytes 处理 hex/utf8/repeat/struct_ 并校验 field size（emit_js/ident.zig:74-109），data 段 active/passive（sections.zig:85-94） | [能力缺失] 中高：数值/聚合常量在 wasm 产物中无声消失 |
| W15 | 指令支持矩阵 | Atomic* 一律 unsupported（lowering `:863-874`），Panic→UNREACHABLE（`:875`），Va*/Native/Nop 空（`:876-881`），V128 诊断（`:608-616`） | mapInstruction 把 borrow/move/release 等显式 `.noop`，stack_alloc/br_null/try_/early_return/native 显式错误，原子/SIMD 有 AtomicExtension/SimdExtension 枚举与错误分类（opcodes.zig:17-34,45-53,528-559） | [能力缺失][诊断缺失] 中高："有意 no-op"与"不支持"混为一谈 |

---

## 3. 解释器（`sa-interp`）

| # | 项 | sars 现状 | sci 对照 | 分类 |
|---|---|---|---|---|
| A1 | fallible 退化为普通值 + 按名一次性 set | `Frame.fallible: HashSet<String>`，Call 后按 return_fallible 入集（exec.rs:12-20,259-271），Try `remove(name)` 即成功（`:367-374`） | `RegValue` 显式携带 fallible/status/from_try（interp/memory.zig:43-56），packFallible/unpackSuccess/requireNonFallible（`:105-162`），try_ 整体原样早返回（interpreter_execfunction.zig:443-454） | [简化模型] 高：同名二次 `?` 误判、非调用 null 当错误 |
| A2 | `?` 对 null 提前走错误路径 | `is_error = Ptr(0)|Int(0)||Bytes(first!=0)`，早返回 `Ok(Value::Int(0))`（exec.rs:375-379），`run()` 强转 i32（`:76-89`） | `?` 非 fallible→InvalidOperand，status≠0 原样返回；runPrepared 映射 status 为退出码（memory.zig:647-666） | [简化模型] 高：demo 372 走错分支（README:52-54） |
| A3 | 整数 i128 + 可选 type mask | `Value::Int(i128)`（value.rs:5-11），bits 缺省 128、mask 按 ins.ty 截断（`:94-112`） | `RegValue{ty,bits}` 严格按 PrimType 宽度解释（interpreter_init.zig:177-249），opBinary 产 i64/u64/f64（interpreter_opbinary.zig:59-87） | [简化模型] 高：缺 as 标注时算术可产生 128 位结果 |
| A4 | 运算按值种类而非静态类型分派 | 任一操作数 Float 即浮点（value.rs:205-225），Slt/Lt 用 i128、Ult 用 u128（`:246-253`），位运算对 float 强转整数（`:234-243`） | numKind 由 RegValue.ty 决定（interpreter_init.zig:406-410），sdiv/and/shl 遇 float 报 InvalidOperand（interpreter_opbinary.zig:110-129,203-235） | [粗粒度] 中高：类型错误被合理化为数值 |
| A5 | 移位宽度 128 vs 64 | `wrapping_shl(y as u32 & 127)`（value.rs:237-242） | `& 0x3f`（interpreter_opbinary.zig:218-235） | [简化模型] 中 |
| A6 | 转换指令粗粒度 | Trunc|Zext 同为 mask、Sext 独立、Fptosi 直接 `as i128`（value.rs:265-271） | trunc/zext/sext 检查宽度与符号扩展、fptosi/sitofp/uitofp 按 target 类型（interpreter_init.zig:436-481） | [粗粒度] 中 |
| A7 | V128 以 4×i32 lane 模拟 | lanes/lane/set_lane 固定 16 字节、Add/Sub/Mul/Shuffle 等实现（value.rs:154-192,287-311） | 解释器显式 UnsupportedInstruction 拒 v128（interpreter_init.zig:414,438-439，opbinary.zig:41,276） | [粗粒度] 中：非 sci 语义，仅 sars 可运行 |
| A8 | 内存模型单一扁平 bump Vec | `Vec<u8>+brk`，alloc 只前进无释放，上限 256 MiB（memory.rs:4,6-58） | Memory.blocks 有序块、allocConst/free/ptrMetaAt/blockMeta、每字节 PtrMeta（interp/memory.zig:75-81,383-543） | [简化模型] 高：无跨块边界/UAF/const-vtable 指针身份 |
| A9 | OOB 错误分类 | read/write 越界一律 `Trap::RuntimeError`（memory.rs:60-87） | sliceAt 校验块归属与范围，返 `RunError.InvalidAddress`（interp/memory.zig:467-483） | [诊断缺失] 中高 |
| A10 | stack_alloc 与 alloc 同构、Release 空 | 两者都 `memory.alloc`（exec.rs:205-209），Release 空（`:308`） | stack_allocs 列表帧退出统一 free，release 跳过 stack/const/interior/borrow（interpreter_execfunction.zig:44-53,110-116,333-355），栈逃逸 Trap（design.md:1001） | [能力缺失] 高：无栈帧生命周期 |
| A11 | 常量/VTable 展平为字节串 | VTable 拼 `"slot:target"` UTF8 字节、只分配裸指针（parse_header.rs:296-309，exec.rs:49-62,571-583） | vtable 物化 slots*8 零字节并写 slot/target 元数据（interpreter_opbinary.zig:286-317，memory.zig:252-269,514-541） | [能力缺失] 高：间接调用无法按槽位解析 |
| A12 | call_indirect 退化为函数数组下标 | `as_u64()`→functions.get(index)，无 provenance（exec.rs:273-295） | 必须取 call_target_name/vtable_slot_name 或 PtrMeta，否则 MissingIndirectCallProvenance（interpreter_execfunction.zig:380-397） | [简化模型] 高：函数指针分派语义缺失 |
| A13 | 地址型 take 只做普通 load | `ins.ty.unwrap_or(I64)`，无 ptr 类型/元数据传播（exec.rs:302-307） | take 默认 .ptr 并传播 interior_ptr/const_name/vtable_slot_name/call_target_name（interpreter_execfunction.zig:117-152） | [粗粒度] 高 |
| A14 | cmpxchg 多目标/成功位无类型 | dest+extra_dests，success 写 `Int(0/1)`（exec.rs:335-352），其它指令 extra_dests 一律复制主 dest（`:417-425`） | ok 为显式第二操作数、写 `.i1`（interpreter_execfunction.zig:208-234） | [粗粒度] 中 |
| A15 | atomic_rmw min/max 错误 | Umin/Umax/Smin/Smax 映射到比较 op（exec.rs:538-551），而比较 op 返回布尔，store 0/1 | atomicRmwApply 正确实现 min/max/umin/umax（interpreter_init.zig:383-405） | [能力缺失] 高：归约结果错误 |
| A16 | 未知 extern 静默返回 0 | has_extern/@/sa_ 前缀判定（exec.rs:132-139），仅 print 三个名字，其余 `Ok(Int(0))`（`:146-165`） | external kind 直接 UnsupportedExtern + stderr（interpreter_execfunction.zig:29-38），syscall 表外 UnsupportedSysIntrinsic（opbinary.zig:479-637） | [能力缺失] 高 |
| A17 | syscall policy/time/fs | 无 policy 概念（exec.rs:146-165） | policyGateAllows 控制 time/fs，sa_time_*/sys_read|write_file 返 SA_STD fallible（opbinary.zig:534-577,605-636） | [能力缺失] 中高 |
| A18 | panic/退出码报告 | Panic→RuntimeError（exec.rs:382-405），run() 返 i32，无 status 通道 | recordPanic exit_code=128+code，DivByZero=100/CallStackOverflow=108（opbinary.zig:411-424，memory.zig:647-666） | [诊断缺失] 高：无稳定退出码契约 |
| A19 | npm/JS 回调队列在解释器缺失 | sa_js_make_callback/poll 一律返 0（exec.rs:163）；JS runtime 句柄为数组下标、无 generation/未排空告警（sa-emit-js/src/runtime.rs:455-493） | generation 化 handle、stale trap、退出未排空诊断（emit_js/runtime_handle.zig:17-35,54-84） | [能力缺失] 中高 |
| A20 | float codeval/f32 类型丢失 | 仅 `Value::Float(f64)`（value.rs:5-11），F32 load/store 靠 `as f32` 临时舍入（memory.rs:94-103,139-146），bitcast 用 f64 bits（value.rs:272-286） | RegValue.ty=.f32、bits 32 位（interpreter_init.zig:214-223,240-248），bitcast 要求等宽（:491-502） | [简化模型] 中：run 与 build-js 的 f32 codeval 可能不同 |
| A21 | 变参指令空实现 | VaStart/VaEnd/Nop 空、VaArg 数组 fallback（exec.rs:362-366,406-415） | va_* 一律 UnsupportedVariadic 并报错（interpreter_execfunction.zig:28,477-481） | [能力缺失] 中：静默通过 |
| A22 | native escape 空实现 | Native 空（exec.rs:362-366） | native→UnsupportedInstruction（interpreter_execfunction.zig:482-485） | [能力缺失] 中 |
| A23 | 未知符号/Undef/Label→0 | 未知 Operand::Symbol→Int(0)，Label/Undef→Int(0)（exec.rs:429-462），空常量返标量（`:480-486`） | 缺寄存器/常量→InvalidOperand（interp/memory.zig:202-222），未知名→InvalidOperand（opbinary.zig:242-257） | [诊断缺失] 中：错误被 0 掩盖 |
| A24 | PtrAdd 不标 interior pointer | `b.wrapping_add(o)` 直接 Ptr（exec.rs:318-322） | ptr_add 置 interior_ptr 影响后续 `!` 是否物理释放（interpreter_execfunction.zig:284-297） | [粗粒度] 中高 |
| A25 | 调用深度/gas 上限 | MAX_DEPTH=256→RuntimeError，step/gas 同为 GasExceeded（exec.rs:10,111-121,167-174） | max_call_depth=1024、专有 CallStackOverflow（interp/memory.zig:41,579-583,647-666） | [粗粒度] 低 |

---

## 4. Frontend：Flatten / 宏 / import / forbidden / @target

| # | 项 | sars 现状 | sci 对照 | 分类 |
|---|---|---|---|---|
| B1 | 宏仅文本 `%param` 替换 | collect_macros 逐行取 [MACRO]（macros.rs:16-57），expand 文本展开（`:59-175`），substitute 裸字符串前缀替换、不感知字符串/注释（`:222-248`） | token 级替换、跳字符串/注释（renderwithtokenreplacements.zig:19-135），`%args...` 变参、`[REP]/[IF]/ELSE`、内建 PRINT!/FORMAT!/STRINGIFY!/CFG!/ENV!/LINE!/FILE!/COLUMN!/INCLUDE!（emitrange.zig:281-467） | [能力缺失] 高：大量 std 宏源无法按 sci 语义展开 |
| B2 | 卫生宏仅 L_ 标签 | label_defs 只收 `L_*`（macros.rs:250-262），只改写 label 定义与 jmp/br/br_null 目标（`:264-308`），后缀 `__mN` | 收集 L_ 标签与 `_` 开头寄存器统一加 `__sa_hyg<N>`（renderwithtokenreplacements.zig:145-205，卫生宏.md:119-174） | [简化模型] 高：宏内临时寄存器多次展开冲突 |
| B3 | 重名宏静默覆盖 | `macros.insert(name, ...)` 直接覆盖（macros.rs:55） | 非相同定义报 DuplicateDef、仅逐字节相同才容忍（emitparsedline.zig:635-705） | [诊断缺失] 中高 |
| B4 | 宏展开预算局部 | depth>=max_depth 或同名在栈即错（macros.rs:143-150，lib.rs:113-119） | 全局 max_macro_expansion_events/lines=10M、recursion=256（importexpansion.zig:13-27） | [粗粒度] 低 |
| B5 | `#def` 存原始串不求值 | parse_def 把 tokens join 成字符串（parse_header.rs:61-78），替换时递归文本替换（macros.rs:426-498） | putExpression 计算表达式并规范化为十进制（def_dict.zig:41-67），支持 +-*()、嵌套 def、hex、内联注释（`:172-338`） | [能力缺失] 高：`#def A = 1+2` 在 sars 不是 3 |
| B6 | #def 冲突范围窄 | 只在 imports::merge 比较 imported def 原串（imports.rs:17-33），同文件重复取第一项无诊断（macros.rs:389-391） | 所有 .def 行进 putExpression，值不同即 DuplicateDef（def_dict.zig:41-67） | [诊断缺失] 中高 |
| B7 | 地址中 #def 偏移靠重解析 raw | Address::parse `parse::<i64>().unwrap_or(0)`（instr.rs:438-453），expand_program 替换 raw 后仅对列出的内存指令 copy_address（macros.rs:310-324,344-357） | parseLayoutOffset 折叠 def、qualify、后缀匹配，失败即 InvalidSyntax（renderwithtokenreplacements.zig:231-293） | [粗粒度] 中：失败静默保留 0 偏移 |
| B8 | import 无包身份/pin/版本 | FsResolver 目录列表+逐段 shedding（resolver.rs:45-77,93-121），根来自 CLI 与 SA_STD（pipeline.rs:49-96），默认 resolve_child 忽略 allow_roots（resolver.rs:10-26） | resolveImport 标准/相对/插件根/包身份并校验 pin ref 与 SHA-256（pkg/resolver.zig:744-811），模块定位器域/扩展名/点分规范（module_resolver/locator.zig:18-24,85-177） | [能力缺失] 高：供应链语义全缺 |
| B9 | 相对路径逃逸粗粒度 | find_local_path 每 `/` shed 一段，允许 `east/../west/beta.sa`（resolver.rs:62-78,188-203） | normalizeSpecifier 对 `..` 直接 InvalidImportSpec（locator.zig:146-151），validateIdentity 拒 `../`（pkg/resolver.zig:216-219） | [粗粒度] 中 |
| B10 | 循环导入诊断类别/信息 | ForbiddenSyntax + 固定文案（lib.rs:98-108） | error.ImportCycle + cycle_path（flattener/expandimportsinto.zig:111-115，module_loader/cycle.zig:195-208,265-308） | [诊断缺失] 中：无环路径、类别误导 |
| B11 | 嵌套失败无源归因 | 宏诊断用 SourceLoc::unknown()（macros.rs:73-77,410-424），嵌套解析失败仅追加 parse diags（lib.rs:129-136） | ErrorContext 保 origin_path/origin_line/detail（importexpansion.zig:127-200），逐行 recordErrorOrigin（expandimportsinto.zig:87） | [诊断缺失] 高：跨文件宏错误无法定位 |
| B12 | forbidden-syntax 粗粒度字符串门禁 | PATTERNS 仅 `; { } => let/fn/impl/.../if (`（forbidden.rs:3-6），只查 Instruction.raw（`:8-30`），parser 还会 sanitize_keyword_operands 改写 raw（parse_instr.rs:342,367-419） | 词法级检查 {} 全词与 a.b.c 属性链、跳字符串/注释、在整个展开源上执行（flattener/forbidden.zig:3-118） | [粗粒度] 高：裸 if/else 与属性链漏检 |
| B13 | @target 未接入主流程 | select_target_lines 只按注释 @target-* 过滤（parser.rs:67-110），仅 parse_source_with_target 使用（`:52-65`），flatten_source 调 parse_source 默认 native（lib.rs:254,261），CLI 直接 flatten_source（pipeline.rs:98-109） | decl_materialize 严格 materialize begin/end/section，重复/未知/空段/嵌套/未终止均报错（decl_materialize.zig:24-146,172-263），root 与 imported 统一（expandimportsinto.zig:405-415） | [能力缺失] 高：build-js/wasm 实际拿 native 变体且无诊断 |
| B14 | struct/hex 常量粗粒度 | struct 字段 resize/truncate 静默补齐/截断（parse_header.rs:251-277），hex 逐字符扫描失败塞原字符（`:278-295`），无 repeat: | 字段实际长度须等于声明 size（common/const_decl.zig:362-377,401-417），hex 严格 \xNN（`:311-330`），支持 repeat:（`:344-357`） | [粗粒度] 中 |
| B15 | 两目标 br_null 宽松单目标形式 | 允许 `br_null p, L_NULL`（parse_instr.rs:90-111） | br_null 必须 `cond -> L1, L2` comma-pair（line_classifier_direct.zig:71-84） | [粗粒度] 低中：接受 sci 拒绝的源码 |

---

## 5. Lexer / Parser

| # | 项 | sars 现状 | sci 对照 | 分类 |
|---|---|---|---|---|
| C1 | 字符串转义/UTF-8 校验 | 词法只跳 `\`+下一字符（lexer.rs:263-307），unescape 仅 n/r/t/0/"/\、无 \xNN（instr.rs:372-395） | decodeQuotedBytes 支持 \xHH 并拒未知转义（interp/memory.zig:285-336），utf8ValidateSlice（interpreter_init.zig:145-148，const_decl.zig:333-341） | [能力缺失] 中 |
| C2 | cap 前缀全部剥离 | Operand::parse 循环剥离任意数量 `&/^/*`（instr.rs:289-293） | stripTextOperandPrefix 只剥一个（interp/memory.zig:346-353），区分 raw 引号串与 borrow（`:338-340`） | [粗粒度] 中：非法多前缀被静默接受 |
| C3 | 函数签名解析宽松 | 参数无类型默认 Ptr、无标识符校验（parse_header.rs:148-165），Signature 无 ignored/should_panic（types.rs:187-196） | parseParam 要求 name:ty 且校验标识符（common/signature.zig:215-235），变参必须最后（`:242-269`），@test ignored/should_panic（`:277-299`） | [能力缺失] 中 |
| C4 | `#version` 行类别缺失 | 未知 hash directive 报 invalid #def/...（parser.rs:140-165） | LineKind.version 单列（flattener/line_classifier.zig:6-32,445-451） | [能力缺失] 低中 |
| C5 | `#loc` 解析但不用 | parse_loc 生成 LocDecl push program.locs（parse_header.rs:80-107，parser.rs:150-156），全仓仅 imports::merge/source_file 读，指令 SourceLoc 仍是当前文件行 | #loc→pending_loc 写进指令 upstream_loc 与 LocTable（flattener/emitrange.zig:239-242,667-697） | [能力缺失] 中高 |
| C6 | 未知 token/指令回退 Assign | 未知字节 Other(ch)（lexer.rs:66-73），未知 ident 有 dest 退化为 Assign（parse_instr.rs:299-335） | line classifier 保留 .unknown 交 emitParsedLine/referee 严格处理（line_classifier.zig:6-32，emitparsedline.zig:244-264） | [粗粒度] 中 |

---

## 6. 工程面：CLI / 工具链 / 包管理 / 后端 / 测试

| # | 项 | sars 现状 | sci 对照 | 分类 |
|---|---|---|---|---|
| D1 | 命令集 8 vs 28 | Command 仅 Check/Run/BuildJs/BuildWasm/DumpIr/CompileTokens/Help/Version（cli.rs:10-42），dispatch 一一对应（commands.rs:8-35） | 28 项含 build/workspace/exe/c/obj/lib/bc2sa/init/install/plugin/pkg/cache/fetch/audit/graph/layout/size/test/explain/fix/skills/daemon 等（cli/types.zig:17-47，dispatch.zig:97-134） | [能力缺失] 高 |
| D2 | 编译选项全集缺失 | 仅 --json/--no-verify/--trace/--gas/-o/--format/-g/--entry（cli.rs:106-244） | --jobs/--dce/--no-incremental/--fast-math/--project-root/--profile/--mem-report/--offline/--ci/--default-immutable/权限 flag、-p/-P/--target（help.zig:24-57） | [能力缺失] 高 |
| D3 | build-js 无 --emit-dts | BuildJs 仅 {file,out,format,debug,entry}（cli.rs:22-28） | --emit-dts（help.zig:255，dispatch.zig:500-543，build_js.zig:25-39） | [能力缺失] 中 |
| D4 | build-wasm --entry 空操作、无 target | `let _ = entry;`（commands.rs:180），固定默认 WasmOptions（`:185`，lib.rs:31-39），无 --target | --target wasm32|wasm64（help.zig:243-248，types.zig:117-121） | [简化模型][能力缺失] 高 |
| D5 | 每命令 help/版本主题 | 仅全局 help/version（cli.rs:40-41,104-105） | sa help [command] + printHelpTopic（help.zig:392-425）、每命令长帮助（`:184-408`） | [能力缺失] 中 |
| D6 | project/workspace 与语言前端路由 | 无 project root，resolve_roots 向上 5 层找 sa_std+cwd+SA_STD（pipeline.rs:70-96） | routeProjectVerb/routeLanguageFrontend（cli.zig:83-105，cli/route.zig:33-53），build_workspace（dispatch.zig:185-228） | [能力缺失] 高 |
| D7 | check 语义差异 | compile(file,!no_verify) 成功打印 OK（commands.rs:68-85），--no-verify 跳 verifier（pipeline.rs:110-117） | 用 verdict cache 跳重复验证、--emit-sab（help.zig:330-339，cli/check.zig） | [简化模型] 中：check 可退化为纯 parse |
| D8 | run 参数模型 | 固定 --trace/--gas、不接受 argv（cli.rs:129-162，commands.rs:92-96） | `sa run <file> [compile-options] [args...]`（help.zig:266-272，dispatch.zig:239-260） | [简化模型] 中 |
| D9 | compile-tokens/dump-ir 口径 | compile-tokens 只输出词法 token 数+functions+instructions（commands.rs:204-229，json.rs:45-47） | compile_tokens = flat.instructions.len+const_decls.len+function_sigs.len+test_sigs.len+verified.annotated.len（cli/compile.zig:38-41） | [粗粒度] 中：指标不可比 |
| D10 | gas 静态估算 | gas_estimate=Σ gas_cost（pipeline.rs:121-128），标为 "gas used" | gas 列为 Referee/debug trap、roadmap-only（docs/errorcode.md:44,90），编译期 token 有独立 metrics | [粗粒度] 中：循环/调用不计入 |
| D11 | JSON 输出面 | 仅 diagnostics_json/check_json/tokens_json（json.rs:25-47），diagnostic JSON 丢 hint/function/register（`:28-34`），--json 仅 check 可用（cli.rs:114-116） | 全局 --json（cli.zig:9）、统一 TrapReport schema（errorcode.md:5-8）、status/metrics/diagnostics（agent_first_toolchain.md:28-50） | [能力缺失][诊断缺失] 高 |
| D12 | daemon 整体缺失 | 9 crate 中无 daemon/socket/NDJSON | sa daemon、Unix socket worker、ping/shutdown/cancel、generation、配额、metrics（cli/daemon.zig:28-161），SA_DAEMON_SOCKET 等（cli.zig:120-132） | [能力缺失] 高 |
| D13 | pkg 全栈不存在 | 无 pkg crate，仅 FsResolver::find_path + find_local_path（resolver.rs:45-78） | src/pkg/ 15 模块（resolver/audit/ci/fetch/lock/sum/workspace/mirror/manifest/confirm），URL=身份、SHA-256 钉版、零权限默认、拒预编译产物、postinstall 不执行（package_management.md:39-60） | [能力缺失] 高 |
| D14 | 增量缓存 | 每次全量 read_to_string+flatten+parse+verify，无 cache 命令/目录/--no-incremental（pipeline.rs:98-119） | 文件级增量默认启用、build-obj-incremental 复用 per-function .o、sa cache clean（help.zig:126-144），src/incremental_cache/ facade | [能力缺失] 高 |
| D15 | DCE/reachability/依赖图/graph·size | 无 dce/reachability/dependency_graph 模块与 --dce，所有函数进所有后端 | DceMode{no,std,full}（emit_options.zig:1-20），reachability/dce/dependency_graph facade，sa graph/size（types.zig:35,37） | [能力缺失] 高 |
| D16 | 并行分析/发射 | 全单线程 | --jobs、src/parallel_analysis/、LLVM emit worker 多线程、test runner 线程池 | [能力缺失] 中高 |
| D17 | 插件系统 | 无 plugin crate，仅 JS #npm_bind | src/plugins/ 17 模块、C-ABI descriptor+dlopen、每插件权限策略、sa plugin install/list/confirm、sa skills | [能力缺失] 高 |
| D18 | SAB/分布式/片段 | 无 .sab 读写/magic/disasm/fragment/跨模块校验/链接器/--emit-sab | sab.zig+sab/*、sab_fragment.zig、encode_fragment、check --emit-sab（help.zig:336,340-354）。注：build --emit-sab 与 SAB 链接器在 sci 侧也未实现（sab_distributed_compilation.md:7） | [能力缺失] 高（引用须同时标注 sci 侧限制） |
| D19 | SAX | 无 SAX 前端；FunctionKind::Gpu 与 Sax* trap 在 enum 空壳（parser.rs:191-197，trap.rs:51-57,122-128,193-209） | src/sax/、四件套文档、DOM airlock 26 API、sa sax build、Sax* trap 族 | [能力缺失]（trap 空壳为 [诊断缺失]）高 |
| D20 | native/C/LLVM 后端 | 仅 JS/WASM+解释器；README:55 自认 "native/LLVM and C backends are not ported" | emit_llvm_llvmc.zig facade + shim.c 227KB/2091 行/246 LLVMBuild*，emit_c.zig+emit_c/ 41 文件，build-c/build-exe/build-obj/build-lib/driver/zigcc | [能力缺失] **极高**：最大能力鸿沟 |
| D21 | build-lib 导出契约 | 无 --contract/C header/API 表 | build-lib 从 .sai 契约生成 C 头+函数指针表+隐藏非契约符号（cli.zig:48-67，help.zig:233-241） | [能力缺失] 中 |
| D22 | #npm_bind 跨后端诊断 | build_wasm 不检查 #npm_bind 直接发射（commands.rs:179-192） | 只有 build-js honors，run/build-exe/build-wasm/build-obj/test 用专用诊断拒绝（readme.md:180-182） | [诊断缺失] 中 |
| D23 | sa test 框架 | 无 test 命令；@test 仅在 parser 识别为 FunctionKind::TestFunc 并写 IR（parser.rs:193，types.rs:240），无选择/执行/断言 | sa test + 11 flag、test_meta/runner/executor/formatter/result、@test 发现→编译→子进程→并行汇总（docs/unit_test_framework.md:36-54） | [能力缺失] 高：@test 语法空壳 |
| D24 | runner/formatter 语义 | 无运行结果模型，仅 cargo 单测 | RunSummary{passed,failed,skipped,ignored}、writeOutcome/writeSummary、[PASS]/[FAIL]/test result: 稳定文本（test_formatter.zig:6-84） | [能力缺失] 高 |
| D25 | 选择性测试 --affected | 无 | affected_tests.zig 两级缓存+函数体 hash 调用图闭包（help.zig:324，dispatch.zig:637） | [能力缺失] 中 |
| D26 | 自带 sa_std | 树中 .sa/.sai/.sab 文件数 0，无 sa_std/ 目录，仅靠 resolve_roots 找外部目录 | sa_std/ 675 文件（92 .sa、~3157 [MACRO]、340KB Zig runtime/359 导出 fn）、prelude、Vec/HashMap/String/Option/Result/Rc/Arc/Mutex（readme.md:260）。注：sci 自报仍有泛型/迭代器/trait 缺口 | [能力缺失] 高：无法独立发布工具链 |
| D27 | 错误码/诊断结构 | Trap::code() 十进制段码 1001-2999（trap.rs:146-215），Diagnostic 有 hint/function/register 但不输出；无 severity/repair/explain/fix/skills | trapCode() 公开码+稳定字符串码 SA-FLAT-001/SA-REF-010/011（errorcode.md:11-18），code/severity/location/repair JSON（agent_first_toolchain.md:28-50），explain/fix/skills 命令 | [简化模型][诊断缺失] 中高 |
| D28 | demo/负例语料覆盖 | 树中 0 demo，只有正向通过率（966/970、913/970、806/970），分母 970 与现树 968 不一致 | demos/rosetta 968 目录，tests/ 15+ 套（cli_smoke/agent_fixtures/golden/js_matrix_smoke/unit_framework/...），963 名集负例/陷阱对拍（DELIVERY.md:193-198） | [粗粒度][能力缺失] 中高：无负例/陷阱/golden 回归 |
| D29 | 模块解析 allow_roots | ImportResolver::resolve_child 忽略 allow_roots（resolver.rs:14-26） | 顶层/子层 root 许可与符号逃逸、symlink 拒绝（pkg/resolver.zig） | [简化模型] 中 |
| D30 | verifier 选项等价性 | VerifyOptions 4 个全局开关（sa-verify/src/lib.rs:14-31），README 称按 Referee 建模但无位掩码/跨函数摘要证据 | 11 态 mask + O(1) 扁平并行表 + 分支快照/lattice；注：sci 侧承认 mergeJoinMask 三条 PATCH 放宽前向边、assume_safe 零校验、"完整内存安全验证过誉"（completeness…:34-36,98） | [简化模型] 高：安全核心等价性不可证 |

---

## 7. 实证条目：R1 wasm 回边/循环控制流（按 10187 登记）

- 现象：B1 落地后 R1 57 名全部 trap（exit 2002），无一自动转绿。
- 根（57/57 单根）：`sa-emit-wasm/lower.rs branch_depth()` 对回边（target 块序号 <= current）返回 None，`emit_branch()` 于是发射 UNREACHABLE，运行期即 trap。逐名 IR 扫描确认 57 名均含至少一条回边（含 54_mem_fill / hashmap_probe / hashset_probe 的 std 内循环，标签前缀非 L_ 也命中）——非 std lowering 缺口，是控制流成环的统一根。旁证：04_loop / 21_while_loop / 536_loop_accum_code / hashmap_probe 在 JS 面 exit 0，属 wasm 专有。
- 分类：**[能力缺失]：wasm 发射器回边/循环控制流未实现**，与 §2.2 W1 同源（此处为实证证据）。
- 修复形态分叉 A/B/C 按 10057 撞分叉停下报主会话，10187 裁定本轮不裁、保持 open 登记；未动手。A=结构化（每回边目标前插独立 LOOP 并重排 BLOCK 嵌套）；B=分派循环（整函数包 LOOP+pc 变量）；C=最小补丁（仅 j==0，覆盖不了 04_loop）。我建议 A，但等用户看完清单重新定界后与清单其他项一起裁。

## 8. 簇外翻转定性（按 10183 纪律 1，待主会话复核）

- B1 报告中随主修复翻绿的 20 名 float/codeval 簇（114/198、476-484/491-494/507/523/531-534/541）为**簇外翻转**，不计入 B1 战绩、不计入任何口径。
- 先验定性候选：共享层 `ConstValue::vtable_target()` 归一化的正当外溢（与 float/codeval 无关，属口径漂移/归因未清），或此前语料名集口径与"禁止简化"新口径之间的漂移。最终归因待主会话复核，未复核前只登记不计数。

## 9. 分类计数（按主条目口径）

- [简化模型]：A1/A2/C1/D1/I2、J2/J3/J7/J10/J11/J12/J15/J18/J21/J22/J24/J25、W0/W2/W12、interp A1-A3/A20/D5/D8/D27/D30、B2/B4、C6、D4/D7/D29 等
- [能力缺失]：B1-B5/C2/D1-D2/E1-E5/G3/H1/I1/I3-I5、J1/J4/J7/J10/J11/J14/J19/J20、W1/W4/W5/W6/W9/W11/W12/W14/W15、interp A8/A10-A13/A15-A17/A19/A21-A22、B1/B3/B5/B8/B13、C1/C3/C4/C5、D1-D8/D11-D28 等
- [粗粒度]：A2/A3/E2/F1/G2/I1、J5/J6/J8/J9/J12/J13/J16/J17/J23、W3/W6/W8/W13、interp A4/A6/A7/A9/A14/A24/A25、B4/B7/B9/B14/B15/C2/C6、D9/D10/D28 等
- [诊断缺失]：A2/A6/F1/G1/G2、J8/J9/J13/J14/J20、W7/W10/W15、interp A9/A18/A23、B3/B6/B10/B11/C5、D11/D22/D27 等

## 10. 重新定界请求

按 10146/10183/10186/10187：修复线整体冻结，R1 A/B/C、JS callback/float/codeval、81 sa_tmp、963/964 校验值一律不开工；请用户据本清单重新定界（先裁哪些项属"真缺口修复"、哪些维持/降级、边界如何划），定界后才恢复任何修复簇。本清单期间未改实现代码、未提交、未扩簇。
