# build-exe 扩面簇中期账（回 11158）

- 时点：2026-11-25 20:29 CST；HEAD 前值 `b15d940`（与主会话同步树一致，工作树干净，无未提交实现）。
- 口径：一簇一提交、pass→fail=0、禁简化；本笔为**纯取证账**，不冒充实现交付，不落凑数战绩。

## 1. 当前在做哪一类 / 已绿 / 还剩

当前类 = **fallible status+value ABI（`?` 解包）**，即 `DELIVERY/BUILD_EXE_SKELETON.md:41-43` 的下一簇清单第 1 项。

| 指标 | 值 |
|---|---|
| 已绿（build-exe rc=0 且 exe rc=0） | **0 / 43** |
| 仍被显式拒（`[E2002] UnsupportedInstruction`） | **43 / 43** |
| 剩余 | 43（35 fallible 签名 + 8 `?` 解包；逐名见 `DELIVERY/build_exe_fail_classify.json`） |

全簇余量（226 名 build-fail 实测分簇）：vtable/call_indirect **112**、extern/shim **28**、atomics/fence **3**（`108_atomic_spin_lock` / `109_atomic_fetch_add` / `363_fence_noop`）、native **1**；npm_bind **10** 按 D22 诊断对齐不计扩面；另 **29** 名属 verifier/语料/环境（MemoryLeak 15、bitcast width 6、import cycle 2、DuplicateDef 2、zig cc failed 2、其它 2）。

## 2. 卡点

**编译器缺口**（C 后端未实现 fallible ABI），非语料形状、非环境：

- `crates/sa-emit-c/src/emit.rs:589-590` fallible call 显式拒；`:651-654` `Try`（`?` 解包）显式拒；`:683-686` fallible 函数签名显式拒。
- 同一 43 名在 wasm 后端已 **43/43 pass-after、pass→fail=0**（`DELIVERY/B3_FALLIBLE_SIGNATURE.md:37-50`）→ 语料可编可跑，缺口只在 C 后端。
- 逐名诊断采集：`DELIVERY/build_exe_fail_classify.json`（226 名全量 `sars build-exe` 实测，每条 `[E...]` 原文归类）。
- 环境侧仅 `331_rc_shared_ownership` / `333_weak_cyclic_reference` 2 名 `zig cc failed`，与 fallible 无关。

## 3. 能先交的子集

- 严格口径下**当前无已绿的扩面实现子集**可落（0/43），不落凑数 DELIVERY。
- 最短可交路径：`atomics/fence` 3 名（仅缺 C11 `__atomic` / fence 下沉，不动 ABI 结构），一类一提交，附 fail-before/pass-after 日志，pass→fail=0。
- 其次：fallible 中不含浮点/聚合的最小子集，rc 与 stdout 双口径确认后再落。

## 证据

- `DELIVERY/build_exe_fail_classify.json`（本笔新增，226 名逐名分类）
- `DELIVERY/build_exe_fail_before_pass_after.log:969`（SUMMARY total=968 before_fail=968 after_build_pass=742 after_build_fail=226）
- `DELIVERY/BUILD_EXE_SKELETON.md:41-43`
