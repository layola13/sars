# B5 transfer 指令交付（sci armTransfer parity）

- 日期：2026-11-28；执行：zcode（25027，接手 workbuddy sars 线后第一批）
- 依据：派单 #19134（先普查）、改派裁定 #19160（§4 B5 核销、第一批=§1 transfer）、
  方案批复 #19201（混合对齐：规则臂逐位谓词 1:1 + 真值表补 dest 侧行）
- 基线：before = `cf1a275`（接手基线，cargo test 345/0 + rosetta 962/6 双 sha 对版）；
  after = 本笔。oracle：sci `zig-out/bin/sa` + `src/verifier/verify_body_arms_alloc_borrow.zig:251-302`
  armTransfer + `src/flattener/line_classifier_direct.zig:32-44` 文法。

## 语义（sci armTransfer 逐条对齐）

`transfer <container>, <value>`：value 的所有权移入 container。验证器可见注解、
运行时 no-op（前置 store 已完成数据搬运）。检查序：双寄存器操作数 → 容器须 live
owned root（borrow_view/ffi_borrow/interior_ptr 位 → TransferIntoBorrow）→ 不可变
（@const 具名或 immutable 位 → TransferIntoImmutable）→ 容器活跃借用
（BorrowConflict）→ 容器已消费（UseAfterMove）→ 值侧同 `^` 移出 retire 到
Consumed → 容器 |= 布尔 owns_children 位（不计数；deep-drop 配对归 SLA/codegen
DropPlan，sci 注释原文见 #19171 专章）。sars 与 sci 的模型差：被借容器在 sars
记 LOCKED_* 位（无 ACTIVE 位），sci 记 active|locked——故 sars 侧 LOCKED 测试先于
ACTIVE 测试，保证该状态报 sci 的 borrow_conflict 报文。

## 改动面

- `crates/sa-ir/src/instr.rs`：`InstructionKind::Transfer { container, value }`
  变体 + reads()/to_text()/gas_cost()。
- `crates/sa-parser/src/parse_instr.rs`：`transfer` 助记符分支（逗号对两寄存器）。
- `crates/sa-verify/src/rules.rs`：Transfer 臂（谓词 1:1 对齐 + 具名 @const 检查）。
- `crates/sa-verify/src/mask.rs`：TRUTH_TABLE 20→28 行，唯一合法 TransferOwn 行
  new_mask 由源侧伪值 0x08 修正为 dest 侧 0x401（ACTIVE|OWNS_CHILDREN），补齐
  sci 四种报文对应的 dest 掩码行（0x08/0x02/0x04/0x20/0x200/0x11/0x100）。
- `crates/sa-flatten/src/walk.rs`：rewrite_operands 补 Transfer 双操作数改写。
- `crates/sa-interp/src/exec.rs`、`sa-emit-js/lower.rs`、`sa-emit-wasm/lower.rs`、
  `sa-emit-c/emit.rs`：no-op 臂（sci 三后端同口径，零行为变更）。
- 单测：parser transfer 解析 1 例；verifier 五分支（合法/进借用/活跃借用占用/
  进已消费/非寄存器操作数）+ 真值表逐掩码覆盖四种报文（批复②要求）；
  A1a/A1c 两个既有断言按批复口径更新（20→28 行、0x08→0x401 修正）。

## fail-before / pass-after

| 项 | before（cf1a275） | after（本笔） |
|---|---|---|
| case5_transfer.sa `transfer x, y` | `[E2002] UnsupportedInstruction: unsupported instruction transfer` | sars check/run rc=0，sci rc=0，双边一致 |
| case6 进借用容器（transfer b, y，b=&x） | E2002 | 双边 rc=1 TransferIntoBorrow，报文逐字同 |
| case7 进 @const（transfer K, y） | E2002 | 双边 rc=1 TransferIntoImmutable，报文逐字同 |
| cargo test --workspace | 345/0 | 351/0（新增 6 例 + 2 处既有断言按批复更新） |
| rosetta 968 | rc0=962 sha 830304a6…2922 / rc1=6 sha 8f89c17e…d55 | 同左，零翻转（语料无 transfer 用例） |

复现件：`/tmp/b5_probe/case{5,6,7}_*.sa`（仓外），探针口径 sars=target/release、
sci=zig-out/bin/sa，双边 check+run 四向 rc 对照。

## 红线遵循

A3/E2 冻结未触（transfer_own 底座与本次规则臂均不在冻结面）；B6（同文件异值
#def 重定义静默放行）只登记未动，按 #19201 定序排在 transfer 之后；本地 commit、
未 push（禁推令 #19017）。
