# 零页簇 — JS bump 基址 0→65536（回 10784 裁定，走甲）

## 根因与修复（非简化，对齐底座语义）
`crates/sa-emit-js/src/runtime.rs:11` 原为 `let SA_BRk = 0;`，从地址 0 起 bump：
第一个常量即落在 0，与 hashmap/hashset 的 `key = 0` 墓碑哨兵冲突。
改为 `65536`，与 sci `src/emit_js/runtime_header_memory.zig:19` 的
`let __sa_brk = 65536; // bump allocator starts after reserved zero page region`
同一判据——保留零页，地址 0 回归真 null；不是墓碑绕行、不是简化桩。
只改这一行：`sa_alloc`（`:50-56`）与 `sa_stack_alloc`（`:57-62`）共用同一 `SA_BRk`，无其他直接相关处。

## wasm 侧核查（落地条件③：明示已核）
wasm runtime **无同样的 0 基址假设**，故不并入本簇、未跑 969：
- `crates/sa-emit-wasm/src/lib.rs:35` `stack_size: 65536`；`:165-169` `heap_start = max(1024, stack_size) = 65536`。
- `crates/sa-emit-wasm/src/lower.rs:1818-1822` 入口把 `heap_start` 写入地址 0 的 brk 槽；
  `:1459-1483` `Alloc` 从该槽加载并回写，返回值始终 ≥ 65536。地址 0 只是 brk 槽位，不是堆指针。
- 常量布局 `lib.rs:49` 从 cursor 1024 起，亦不占用 0。

## 证据（973 JS 全量；before = 本簇前 HEAD bf11518 二进制快照，after = 本笔）
- **before 943/973，after 944/973**；**pass→fail = 0**；fail→pass = 1
  （`hashset_probe`：before rc=1 `error` → after rc=0 `set ok`）。
- `cargo test --workspace` 全绿（209/209）。
- 取证件：`/tmp/opencode/zp_harness.py`（由 `DELIVERY/e_harness.py` 仅替换 before 二进制与临时目录），
  结果 `/tmp/opencode/zp_js_compare_results.json`、摘要 `/tmp/opencode/zp_full_log.txt`。
