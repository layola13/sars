# A1 完整语义实施预案（等主会话裁定，未动实现）

## 目标形态

在 `sa-verify` 建立与 sci `common/capability.zig:3-19` 逐位一致的 12 位状态模型，
并以 `referee/table.zig` 的真值表为唯一转移依据；`RegState`/`RegKind` 的并行
HashMap 与 `fallible`/`borrow_origin`/`origin_base`/`escaped`/`borrow_params`
五张旁路表收敛为「每寄存器一个 u16 mask + 必要的来源元数据」，诊断携带
`maskName` 名字与前后 mask（`sax_trap_report.zig` 口径）。

## 12 位映射与当前缺口

| 位 | sci 名 | sars 现状 | 接入点 |
|---|---|---|---|
| 0x001 | active | `RegState::Owned{..}` | `state.rs:3-11` / `rules.rs:78 define` |
| 0x002 | locked_read | `Borrowed{shared>0,mutable:false}` | `rules.rs:202 add_borrow` |
| 0x004 | locked_mut | `Borrowed{mutable:true}` | `rules.rs:202` |
| 0x008 | consumed | `Moved`/`Released` 合并为一个语义 | `rules.rs:137 consume`、`311 release` |
| 0x010 | borrow_view | 无；`borrow_origin` 仅字符串旁路表 | `state.rs:28` / `rules.rs:202` |
| 0x020 | ffi_borrow | 无；`Capability::FALLIBLE` 只用于文案 | `rules.rs:408 apply_cap` |
| 0x040 | untracked | 无 | `rules.rs:44 check_redefine` 的 Scalar 豁免 |
| 0x080 | fallible | `fallible: HashMap<String,bool>` 旁路表 | `state.rs:28` / `rules.rs` Try/Call |
| 0x100 | immutable | 无；const 只做 `is_constant` 字符串查询 | `rules.rs:89 is_constant` |
| 0x200 | interior_ptr | 无 | `PtrAdd` 目前落 default |
| 0x400 | owns_children | 无 | `transfer` 在 sars 无对应指令 |

## 真值表落点（sci `TRUTH_TABLE` 20 条）

`alloc / borrow_read / borrow_mut / borrow_read_again / move_ / release_borrow /
release_own / ffi_raw_cast / ffi_assume_safe / ffi_assume_borrow / transfer_own`
共 11 个 Op × 12 个典型前置 mask。sars 须把 `rules::transfer` 的分派改为
「先算 prev_mask → 查表得 legal/new_mask/trap → 落 trap 或更新 mask」，
旁路元数据（origin/base/escape）只作附件，不参与合法性裁决。

## 分期与逐项验收（裁定后执行）

1. **A1-a 状态载体**：新增 `u16` mask 类型 + `maskName()` + 真值表常量；
   fail-before 单元证据 = 当前 `FnState` 无法表达 `0x101`（active|immutable）
   与 `0x33`（ffi_borrow|borrow_view）组合，新测试在旧代码编译期即失败。
2. **A1-b 分配/借用/移动/释放四组转移**：逐条真值表项给
   fail-before（旧规则误接受 read_write_conflict / double_mutable_borrow /
   use_after_move / borrow_conflict）→ pass-after 单元测试；不动调用/FFI。
3. **A1-c fallible/immutable/interior/owns_children 位**：分别对应 D1/B3/E1/B5，
   每项单独 fail-before/pass-after，避免把下游项冒充 A1 战绩。
4. **A1-d 合并格**：带 mask 的 label 快照 + 分支冲突 trap（当前仅 warning，
   `lib.rs:100 phi_warning`），对应 A2/A3；每项独立验收。
5. **A1-e 诊断面**：`TrapReport` 式 register/expected/actual mask 名字，
   对应 G1/G2；最后收口，确保前四期不改诊断口径也能独立验收。

## 风险与边界

- 全量改完前 `cargo test --workspace` 与 968 名语料必须保持零回归；
  每期单独跑 `cargo test --workspace` 与 `sars check` 语料回归。
- 不做「简化版 mask」：任何位只用旁路 HashMap 模拟、真值表用 if-else 近似，
  均视为降级实现，停该项上报。
- 与清单 §10 冻结项（R1 A/B/C、JS callback/float/codeval、81 sa_tmp、963/964）
  无交集；是否解冻由主会话另裁。
