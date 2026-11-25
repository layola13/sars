# D 簇 — JS `sa_tmp` 未归一（回 10754 ④ 确认开工，落地序 D→E→C(wasm)→C(JS)→F→B）

## 根因与修复（非简化，完整语义）
`crates/sa-emit-js/src/lower.rs` 的 `dst()`：有 dest 时返回 `FuncCtx::reg_name(d)`（即 `r_<name>`），
无 dest 时却返回裸字符串 `"sa_tmp"`。而 `build_ctx` 把 `sa_tmp` 登记为寄存器、发射时声明的是
`let r_sa_tmp = 0;`。ESM strict 模式下对未声明标识符赋值触发 `ReferenceError: sa_tmp is not defined`。
修复：无 dest 分支改为 `FuncCtx::reg_name("sa_tmp")`，与声明侧同一归一化入口。不改语义、不加桩、不静默。

新增单测 `no_dest_writes_use_normalized_sa_tmp`：断言发射文本只含 `r_sa_tmp`、不含裸 `sa_tmp`。

## 证据（973 JS 名 = 968 rosetta + 5 support；before = 210a3c1 二进制，after = 本笔二进制）
- fail-before：`81_kv_store` rc=1，stderr `file:996  sa_tmp = r_entries;  ReferenceError: sa_tmp is not defined`
  （与 10695 记载的 emitted:996 一致）；`hashset_probe` rc=1、stdout `error\n`。
- pass-after：`81_kv_store` rc=0，stdout `5\n`；973 名逐名对拍见 `d_fail_before_pass_after.log`。
- **before 941/973，after 942/973**；**pass→fail = 0**；fail→pass = 1（`81_kv_store`）。
- `cargo test --workspace` 全绿（208/208，含新增单测）。取证件：`d_harness.py`。

## 遗留（新增发现，非 D 根因，报请裁定，不自行扩簇）
`hashset_probe` 在 before/after 均为 rc=1 / `error\n`，sa_tmp 修复对它无影响：该程序发射文本中的裸
`sa_tmp` 只出现在 `sa_vec_*` 等未被调用的辅助函数里，执行路径未触碰。
真根因另在：sars JS 运行库 `runtime.rs:11` 的 `let SA_BRk = 0;` 从地址 0 起 bump，
第一个常量 `&KEY_A` 的地址即 0；而 sci JS 运行库
`src/emit_js/runtime_header_memory.zig:19` 明确 `let __sa_brk = 65536; // bump allocator starts after reserved zero page region`。
hashset/hashmap 墓碑用 `key = 0` 作空值哨兵，`KEY_A` 恰在地址 0 → 第二次 `SET_REMOVE` 把墓碑当命中，
`removed_again = true` → `L_ERR` 打印 `error`。运行期 trace 证据（节选）：

    PROBE_KEY_CHECK hash=0 slot_hash=0 key=0 slot_key=0
    DEL hash=0 key=0 cap=8 len=2 ; DEL_HIT idx=0
    DEL hash=0 key=0 cap=8 len=1 ; DEL_HIT idx=0     <-- 第二次删除误命中墓碑

改 `SA_BRk` 基址会平移全体指针（广泛 collateral 风险），且与 C(wasm)/F 等簇的 extern/ABI 账可能交叠，
按「撞分叉不自行选」纪律单列报请裁定，建议另立簇（zero-page 基址对齐）并独立跑 973 pass→fail=0。
