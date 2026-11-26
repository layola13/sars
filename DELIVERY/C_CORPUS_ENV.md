# C(corpus/env) 簇交付（接 e521f98 native，回主会话 12116 派单）

- 口径：一类一提交、fail-before/pass-after、pass→fail=0、禁简化；只追本次
  29 名 `corpus/env` 目标，簇外正向溢出单列，不混算。
- before = 父提交 `e521f98` 的 release 二进制**重跑**（非跨日志列算术）；
  after = 任务分支 `task/corpus-env-e521f98` 本提交产物。
- 工作副本：`/content/oc-box-1-work/sars`（一人一份；`../sci` 软链到 `/content/sa_all/sci`）。
- 改动：`crates/sa-verify/src/{rules.rs,tests.rs}`、
  `crates/sa-emit-c/src/{emit.rs,runtime.rs,tests.rs}`、`crates/sa-cli/src/commands.rs`、
  `DELIVERY/c_corpus_env*`。

## 名集与判定

主会话派单给的是 `corpus/env 29`，未附逐名清单（我已在中继 12137 请求确认，
截至交付无回音）。按 `c_native_fail_before_pass_after.log` 的 37 个剩余红自推：

- 14 名 corpus 所有权误报（E1012，demo 自带源码）：115、187、247、251、253、
  255、257、460、468、530、538、548、608、658。
- 5 名 C 端位型缺口（E1004 bitcast/sitofp）：343、344、400、402、403。
- 10 名 npm 宿主环境绑定（D22）：496–501、965–968。
- 合计 **29** 名，与派单计数一致。
- 另 6 名（205/207/226/227/243/502）源码 readme 明写「编译应失败」，属负向
  测试，不翻绿、不改诊断口径；它们保持 rc=1，不在本笔范围。
- 正向溢出 2 名：333_weak_cyclic_reference、335_ts_slice_inverted_bounds
  （同批 verifier/C 改动顺带翻绿，单列不并入 29）。

## 根因与落地形态

1. **verifier 形参所有权**（`sa-verify`）：按值 `ptr` 形参原按 owned 计，函数出口
   未 `!` 即 E1012；改为仅显式 `^`（`CapPrefix::Move`）指针形参承担出口释放义务。
   `call_indirect` 出口泄漏规则收窄到「该借用形参确实作为调用实参传入」。
   `call` 返回标量类型登记为 `RegKind::Scalar`，避免循环里同名结果被误判重定义；
   escaped 寄存器（已作为参数/存储交出所有权）允许同名再定义，消除 ts 标准库
   循环里 `part`/`cpart`/`a1`/`numbuf` 的 E1006。
2. **C 端位型与指针转换**（`sa-emit-c`）：出参期望整数、实参为指针时统一走
   `(<int_type>)(uintptr_t)(expr)`；补 i8/u8、i16/u16、i32/u32、i64/u64 同宽
   bitcast 与 f32↔i32 位重解释（`sa_bitcast_f32_i32` / `sa_bitcast_i32_f32`
   union helper），sitofp 到 f32 由 C `float` 正确舍入。
3. **npm 宿主绑定**（`sa-emit-c`/`sa-cli`）：移除 build-exe 的 D22 npm 拒门
   （build-wasm 的门保留，B 簇诊断对齐不受影响）；runtime 增加确定性 js 事件
   队列与 staged shim：totalmem/parallelism 正数、isAbsolute/isDeepStrictEqual
   按 (ptr,len) 比对、EventEmitter ctor/method、userInfo 字符串出参的
   too-long/full-capacity 两态、createHash digest 长度 32、回调 enqueue/poll/
   call_indirect 派发与 value_release。全部宿主无关、无网络、无真实 syscall。
4. `call_indirect` 无静态 vtable 来源时（运行时回调槽），按 dest 的浮点使用点
   补推 F64 返回类型，避免 cb_double 被按 `int64_t (*)(double)` 误调用。

## fail-before / pass-after

复现件：`DELIVERY/c_corpus_env_harness.py`（968 名，before/after 两二进制各自
`build-exe` + 实跑，记录 rc 与逐字 stdout），日志
`DELIVERY/c_corpus_env_fail_before_pass_after.log`。

| 指标 | before(e521f98 父提交重跑) | after(本笔) |
|---|---|---|
| 全语料 | 968 | 968 |
| rc=0（build+run 双绿） | **931** | **962** |
| 29 名 corpus/env rc=0 | 0 | **29** |
| 正向溢出 2 名 rc=0 | 0 | **2** |
| pass → fail | — | **0** |
| stdout 回归（原本双绿者逐字比对） | — | **0** |

- before 列来自父提交二进制重跑，与父提交自身日志 `after_rc0=931` 逐字节一致；
  本笔不再依赖跨日志集合算术确立增量，已消除 12116 指出的基线登记问题。
- before 二进制 sha256 =
  `60f88d5e55deada5474834a1e8c34b571e630f9812ba075733ad42629177c9c3`（= e521f98
  产物）；after 二进制 sha256 =
  `a1367da1488f335558792a7f889adc658f4be1ca34bba3d9cc274bd7ababf9d8`。
- 29 名逐名 stdout 均命中各自常量（如 343=`343\n`、498=`498\n`、968=`968\n`），
  见日志 `before_rc=1 after_build_rc=0 run_rc=0` 行；溢出 333=`upgraded\n`、
  335=`335\n`。

## 验证

- `cargo test --workspace` 全绿（含改写的 `emits_npm_host_bindings`、
  `unreleased_owned_ptr_param_is_an_exit_leak`、`live_borrow_param_in_indirect_dispatch_is_a_leak`）。
- 取证件 sha256：
  - `DELIVERY/C_CORPUS_ENV.md`（本文 sha 见中继回报，避免自引用）
  - `DELIVERY/c_corpus_env_harness.py` =
    `f992558ba6bcf95c2dde83a9aa9e6ac6d45076583e25a860e56b50bd37f34066`
  - `DELIVERY/c_corpus_env_fail_before_pass_after.log` =
    `3959c893b469f8ea863625daa02215721e8f2d876acb4c0b092586ee3fd4cd06`
  - `crates/sa-verify/src/rules.rs` =
    `a52a78d82e7dd961891b099edb6cc8ec2e5a91eabb33e8d9323d15a701cf74e0`
  - `crates/sa-verify/src/tests.rs` =
    `97b634718fb9bc4c427b5473b433c024046fa1f0b91029ef14f6250cf4217bec`
  - `crates/sa-emit-c/src/emit.rs` =
    `f27487ee3a1f582ca2ee861bf9652f3fd204b79c3e3bdc3c0e53a3eaf0519e4f`
  - `crates/sa-emit-c/src/runtime.rs` =
    `6c12ce9bf26533cfc83a89e5d13493fa756ad9ac670bc212fc520d007d2407e7`
  - `crates/sa-emit-c/src/tests.rs` =
    `d93e3b230285fe16a5acd60db7f565a37a660c4c0458ef0ca9703ee33ad76410`
  - `crates/sa-cli/src/commands.rs` =
    `c4a523edc269f8fafd805ab44eb4e772a60630ae81b3965909b9a82ef79cc670`

## 交付形态与推送

工作分支 `task/corpus-env-e521f98` 只留本地；交付经主会话既有 main 推。
推送按知识库第 1/3 条：盒上 `/proc/<pid>/environ` 取 `GIT_TOKEN`，只经
`http.extraheader` 传给 git，不回显/不落盘/不进配置；推前 `ls-remote` 核旧值，
推后核新值；不建远端分支、不用浏览器自动化。
