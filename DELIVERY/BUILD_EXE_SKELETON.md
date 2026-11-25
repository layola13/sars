# CLI build-exe — first structural increment (方案已交 10695，本笔落地骨架)

## 范围
按 10661 ②「第一个结构项 = CLI build-exe」落地**骨架+单测+链接驱动**，不是全 IR 覆盖：
先让「能安全降位的程序」端到端产出真原生 exe，其余一律显式诊断拒绝，不用零值/空实现冒充。

- 新 crate `sa-emit-c`：`emit_c(&Program, &COptions) -> Result<String, Vec<Diagnostic>>`，手写 Buf、无第三方 crate、单文件 <1000 行。
- `sa-cli` 增 `build-exe`（`build` 为 alias）与 `dump-c`（只出 C 不链接，便于取证）。
  flags：`-o/-g/--no-debug/-O0/-O2/--target/--entry`；`--incremental/--profile/-p` 显式拒。
- 链接驱动 `native.rs`：ZIG env → PATH → `/tools/zig/zig`，`zig cc <emitted.c> -o <out> [-O2|-O0] [-g]`；
  stderr 转 Diagnostic；cross-target 首版显式拒；`#npm_bind` 走 D22 拒绝门；非 main entry 拒绝。
- C runtime shim：bump 分配（16 字节对齐、越界 `sa_panic`）、`sa_print_bytes` 走 `write(1,…)`、
  `sa_panic` 走 `_Exit(1)`、除零守卫、i64↔f64 bitcast 用 union。

## 降位（非简化，已修的真实语义点）
- 常量：`ConstValue::bytes()` 扁平聚合 + utf8/bytes 内联 operand 全部落成 `static const unsigned char[]`；
  vtable const → 显式拒绝（非 0 桩）。
- 寄存器类型推断：params → locals → dest（ins.ty / 首操作数 / call sig）三段式；赋值按 dest 声明类型发射，不按字面量默认类型。
- 转换：`zext/sext` 经源宽度无符号/有符号中间类型；`trunc` 按目标符号；`sitofp/uitofp/fptrunc/fpext` 按目标浮点宽度。
- 比较/算术：`udiv/urem` 无符号、泛型 `div/rem` 按首操作数符号、`shl/lshr` 按源宽度无符号、`ashr` 按源宽度有符号、
  泛型 `shr` 按源符号分流；混合符号同宽比较按位模式（uintN）对齐。
- 调用返回值被丢弃时仍发射调用语句（曾导致 guard_drop / make_some 被优化掉，已修）。
- 不支持项一律 `Trap::UnsupportedInstruction` / `UnsupportedType`：fallible 签名与 `?`、call_indirect/vtable、atomics/fence、
  native/variadic、v128、无 shim extern、cross-target。无静默错值。

## 证据（全 968 名，before = 57bc34f 原二进制，after = 本笔二进制）
- fail-before：`build-exe` 命令不存在 → **968/968 FAIL（rc=2）**。
- pass-after（build）：**742/968** 编译并链接出可执行；226 名按上述诊断显式拒绝（非静默败）。
- 742 个 exe 全部 rc=0（无运行期 trap / SIGFPE）。
- 对照 sci 原生 `build-exe`（`sci/zig-out/bin/sa`，PATH+ZIG 指向 /tools/zig/zig）逐名比 exit+stdout：
  - **MATCH 713**，**真不一致 0**；
  - 6 名（338/347/383/384/388/389）sci 自身 rc=1/空 stdout（已知 sci u64 有符号宽度 bug），sars rc=0 且 stdout 正确输出各自编号 → 按已知 sci bug 单列、不计战绩；
  - 23 名 sci 参考不可构建（REF_SKIP），单列。
  - **验收名集口径（回 10700 ④）：968 名中排除 388/389**（sci 自身 u64 宽度 bug 致其参考输出为空，sars 不与 sci bug 对齐战绩）；同理单列不计的还有 338/347/383/384。计入战绩的验收名集为 968 − 6 = **962 名**。
- `cargo test --workspace` 全绿（新增 sa-emit-c 6 项、cli build-exe 解析 1 项）。

取证件：`build_exe_fail_before_pass_after.log`（逐名 before/after）、`build_exe_reference_compare.log`（逐名 stdout 对拍）、
`build_exe_harness.py`、`build_exe_compare_harness.py`。

## 未覆盖 / 下一簇（待裁）
1. fallible status+value ABI（35 名）+ `?` 解包（8 名）；2. vtable/call_indirect 函数指针表；3. C11 atomics；
4. 无 shim extern 下沉为 shim 集；5. native/variadic。以上均已显式诊断，不会静默错值。

## 账目对齐（回 10700 ⑤ · 57bc34f 确切名集账）

逐名证据：`/tmp/opencode/cluster_full_log.txt`、`r2_full_log.txt`、`b2_full_log.txt`、`b4_full_log.txt`。

- **两套分母分别列**：
  - wasm 分母 **969**（968 rosetta + once_probe）：generic runner rc=0 **919**；typed runner rc=0 **924**；
    严格 stdout 命中 **922**（924 减 301/302）。
  - JS 分母 **973**：pass **941**，fail **32**（与 10695 报数一致）。
- 10673 的「919→924」= R2 typed-stub 落地时的 rc=0 增量（5 名）；10695 的「919/969」= generic runner 的
  rc=0 基线（B4 之后、R2 之前）。两者口径不同，并非矛盾；10695 把基线 919 误写成“现测”属笔误，
  现测 typed rc=0 应为 **924/969**。
- **差额 5 名逐名交代**（R2 中 rc 由非 0 翻 0）：
  1. `186_sqlite_c_api_binding` — stdout `8\n` → 计入
  2. `963_compress_checksum` — stdout `ok\n` → 计入
  3. `964_multipart_checksum` — stdout `ok\n` → 计入
  4. `301_http_client_saasm` — stdout `error\n` → **不计**（输出分歧纪律，仅 rc 假翻绿）
  5. `302_http_server_saasm` — stdout `error\n` → **不计**（同上）
  ⇒ stdout 级净增 **3**，pass→fail=0；rc 级 919+5=924，严格 stdout 级 919+3=922。
- **301/302 的计与不计**：rc 层面计（确实翻 0），战绩层面不计（stdout 仍 `error\n`）——此即 5 与 3 的差。

推送：本机无 GIT_TOKEN，照旧由主会话代推；报本地头后请核远端。
