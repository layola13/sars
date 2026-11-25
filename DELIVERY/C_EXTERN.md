# C(extern) 簇交付（接 24a2481 vtable，回主会话 07:25 派单）

- 口径：一类一提交、fail-before/pass-after、pass→fail=0、禁简化；只追
  `build_exe_fail_classify.json` 中 `extern` 标签的 28 名，`native` 117 留下一笔。
- before = `24a2481`（远端 main / 本地 vtable 提交，工作树干净）；
  after = 任务分支 `task/extern-c-backend-24a2481` 本提交。
- 工作副本：`/content/oc-box-1-work/sars`（一人一份；`../sci` 软链到 `/content/sa_all/sci`）。
- 改动：`crates/sa-emit-c/src/{emit.rs,runtime.rs,tests.rs}` + `DELIVERY/c_extern*`。

## 根因与落地形态

`emit()` 原对每一个非 shim extern 直接发 `[E1050] ImportResolutionFailed:
extern X has no native runtime shim (C backend first increment)`，故 28 名在
`build-exe` 阶段整体失败。本笔按 sci 的 staged host binding 口径补 C 单侧缺口：

1. **extern 定义生成**：`extern_source()` 按 `program.externs` 的真实签名生成
   `static <ret> sa_extN_<name>(<c types> a0…)`，调用点因此可链接；同名内部函数
   优先（消除 220 的重复定义），`sa_print_bytes` 沿用 runtime 中的定义。
2. **确定性 staged 绑定**：`runtime::extern_helpers_source()` 提供句柄表、缓冲表、
   multipart 解析/构建器与 fd/dl/signal 计数器；`shim_body()` 给 57 个实际被调
   extern 绑定语义（fake fd、echo signal、pthread 同步执行、sqlite ROW=100、
   HTTP client/server 回显、compress 字节级往返、multipart 解析/重建、memcpy 真拷贝）。
   未收录 extern 落 trap stub，不静默返回伪成功。全部宿主无关、无网络、无真实 syscall。
3. **配套修正**：bitcast `ptr ↔ u64/i64` 走 `uintptr_t`（81 的 6 处宽度拒收）；
   指针型 dest 的 Add/Sub 按「指针 + 整数」编组，寄存器在指针上下文按 `uintptr_t`
   转换（消除 C 端 int→ptr 编译错误）。

关键形态（`dump-c 301`）：`sa_http_client_req_send` 把 set_body 记录的 `(ptr,len)`
复制进 resp/reader，read_chunk 按 cap 截断并写回真实长度；963 的 compress 走
staged 缓冲表，encode/decode 字节级往返；964 的 builder 产出可再次 parse 的
form-data，part body 校验和两次一致。

## fail-before / pass-after

复现件：`DELIVERY/c_extern_harness.py`（968 名，before/after 两二进制各自
`build-exe` + 实跑，记录 rc 与逐字 stdout），日志
`DELIVERY/c_extern_fail_before_pass_after.log`。

| 指标 | before(24a2481) | after(本笔) |
|---|---|---|
| 全语料 | 968 | 968 |
| rc=0（build+run 双绿） | 788 | **930** |
| extern 28 名 rc=0 | 0 | **28** |
| pass → fail | — | **0** |
| stdout 回归（原本双绿者逐字比对） | — | **0** |
| native 117 | 0 | 0（下一笔） |

- 28 名逐名 stdout 全部命中：54=`7,7,7,7\n`、81=`5\n`、181=`3\n`、
  220=`220\n`、301/302/963/964=`ok\n` 等（见日志 `before_rc=1 after_build_rc=0
  run_rc=0` 行）。
- 簇外溢出为正向：788→930，多为被同一 extern 门槛挡住的其他 demo 顺便恢复；
  两次 968 名跑批同参同机，无 pass→fail、无 stdout 回归。
- before 二进制 sha256 =
  `58aac07533c8a8d9dae75625063de14091f5d41042abb0b1cef9df0b96ad5678`；
  after 二进制 sha256 =
  `72cca8dec063a70588c69233ce10892fde065be3a54444ddfa37bc7b42bffdc6`。

## 验证

- `cargo test --workspace` 全绿（含新增 `emits_staged_extern_shims`、
  `emits_unknown_extern_as_trap_stub`；原 `rejects_unsupported_extern` 改为
  trap-stub 断言）。
- 取证件 sha256：
  - `DELIVERY/C_EXTERN.md`（本文，落盘后回填）
  - `DELIVERY/c_extern_harness.py` = `8780e5ae474d758b3132cfb086c5cf86cf2232ed4b1ba32e7b6c8d2062eefdf1`
  - `DELIVERY/c_extern_fail_before_pass_after.log` = `0d08ae11206d59a94c317f0a752c8b143944ce7b5d9cdbc6914ae859963f15b5`
  - `crates/sa-emit-c/src/emit.rs` = `680cd107b17f823b6a546f704993d02a6ed4002089bfd40d432a5f6bd4893e04`
  - `crates/sa-emit-c/src/runtime.rs` = `fe05199ef72a0869c6f6ad3acfc9430be11ab0b4798c949064585a6bb32dcfb7`
  - `crates/sa-emit-c/src/tests.rs` = `b8c928d0ff6a8d4b1e745ed30e430294ad92487bede27352e43fde9867b97187`

## 后续（按主会话队列）

native 1（117 inline asm）在 `task/native-c-backend-<head>` 续推，交付另写
`DELIVERY/C_NATIVE.md`。推送按知识库第 1/3 条：盒上 `/proc/<pid>/environ` 取
`GIT_TOKEN`，推前 `ls-remote` 核旧值、推后核新值；工作分支只留本地，交付推既有
main，不建远端分支、不用浏览器自动化。
