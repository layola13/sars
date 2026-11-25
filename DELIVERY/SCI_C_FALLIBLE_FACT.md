# sci 本体对 fallible 43 名的 C 目标行为（回 11172 指定事实）

- 问题：sci 本体在 C 目标上对这 43 名是支持还是同样拒收？决定该簇是
  「修 sa-emit-c 缺口」还是「按 D22 式对齐拒收」。
- 结论：**sci 本体支持（42/43 `build-c` 成功并运行；唯一失败与 fallible 无关）**。
  因此该簇是 **sars 单侧编译器缺口**，不是「sci 同样拒收、按 D22 对齐」的情形。
  禁止默认对齐、禁止静默跳过；应修 `sa-emit-c` 缺口。

## 实测（全部在 /tmp 副本上跑，未改动 sci / sars 工作树）

编译器：`/content/sa_all/sci/zig-out/bin/sa`（`sa 0.1.11-优化`，Zig 0.17.1）；
`PATH=/tools/zig:$PATH ZIG=/tools/zig/zig`。

| 命令 | 结果 |
|---|---|
| `sa build-c <180_try_trait_v2>/main.sa -o /tmp/verify_180.c` | rc=0，生成 C |
| `sa build-c <19_result_question>/main.sa -o /tmp/verify_19.c` | rc=1，`[MemoryLeak] live registers remain at function exit`，非 fallible 相关 |
| 43 名全量 `sa build-c` | **42 pass / 1 fail**（仅 19，MemoryLeak） |
| 43 名全量 `sa build-exe` | **41 pass / 2 fail**（19 MemoryLeak；372_try_br_null_combo 为 LLVM `br_null` 缺口，其 `build-c` 成功） |

`sci build-c` 产物实跑：生成的 42 个 C 文件经 `zig cc -O1` 全部编译、运行 rc=0，
stdout 命中各自期望值（180=`7`、368=`368`、366=`366`、416=`416`、513=`513` 等）。

## 生成 C 中的 fallible 落地形态（证据）

`/tmp/verify_180.c` 逐字片段：

```c
static sa_u64 sa_ok_val(sa_u64 v) { (void)v; return v; }
...
  return sa_ok_val(((sa_u64)2ULL));
...
  { sa_u64 sa_t = (r0); r1 = sa_t; }   /* `?` 解包：错误通道被擦除 */
```

`sci` C 后端把 fallible 返回统一为 `sa_ok_*` 包装、`?` 解包退化为值拷贝
（错误通道在 C MVP 后端被擦除），因此这 43 名在 sci 的 C 目标上可编可跑。

## 源码定位

- C 后端入口：`src/cli/build_exe.zig:488`（`executeBuildC`）、`:502`（`emitCToFile`），
  分派于 `src/cli/dispatch.zig:545`、`:583`。
- fallible 返回降低：`src/emit_c/operands.zig:489-499`（`sa_ok_none()` / `sa_ok_val()`）。
- `Try` / `?` 降低：`src/emit_c/linear.zig:127-135`（`{ sa_u64 sa_t = (<src>); r<N> = sa_t; }`）。
- 其它 fallible 处理：`src/emit_c/emit_fn.zig:587-591`、`src/emit_c/tf_emit.zig:427-431`。
- 原生路径走 LLVM：`src/cli/build_exe.zig:367`、`:462`（372 的原生失败源于此，与 C 无关）。
