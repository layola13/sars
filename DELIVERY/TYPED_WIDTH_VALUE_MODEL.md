# typed-width 价值模型交付：输出分歧 8 名 + R3 18 名合并专项

- 裁定：10567。不开逐个转换补丁；输出分歧按 8 名重定界（336/339/361/362/390/396/400/402），
  R3 18 名（476-477/479/481-484/491-494/507/523/531-534/541）合并进同一专项。
  步骤一（位宽/符号模型本体）与步骤二（转换/Take/f32）同笔落地；禁止简化口径不变。
- 基线：before = `de72e77`（远端已核验头）；after = 本笔。只改
  `crates/sa-emit-wasm/src/{opcode.rs,lower.rs}`，另附 `DELIVERY/` 取证件。
- 复现件：`DELIVERY/typed_width_harness.py`（同机同零值类型化 extern 桩，968 名逐名
  `build-wasm` + wasmtime 校验/实例化/执行，trap=2002、build-fail=2001、runner-exception=2005，
  退出码取最后结果）、`DELIVERY/typed_width_fail_before_pass_after.log`（逐名 before/after）。

## 根因

wasm 发射器用单 I64 寄存器模型，`Registers` 只记 wasm storage valtype，不记 SA `PrimType` 的
位宽/符号；`numeric_opcode()` 对 `Trunc/Zext/Sext/Bitcast/Fptrunc/Fpext` 返回 `None`，且
`conversion=true` 使 `lower_op()` 既不诊断也不发射指令——静默恒等。`Take` 与 `Release/Nop`
同组空实现，取地址值从未 load。F32/F64 共用一个 F64 局部且无 IEEE-754 binary32 舍入，
`bitcast` 无法拿到正确的 32 位位型。同一条「trunc 到 i32 再 zext/sext」在 338 与 339 期望相反，
只有按源/目标位宽与符号归约才能同时满足（清单 W6/W13 + R3）。

## 落地形态（完整语义，非桩）

1. **类型模型**：`Registers` 增 `ptypes: Vec<Option<PrimType>>`，与 storage valtype 平行；
   由签名、局部 hint、指令 `ty`、Call/Try/CallIndirect 的返回签名共同推断。转换类指令的
   storage 由目标类型决定（浮点→F64、整数→I64），不再沿用源 storage。
2. **位宽/符号归约**：`trunc` 取目标宽度低位并按目标符号扩展/零扩展到 i64；`zext` 先按源宽度
   归零、再按目标宽度归零（窄目标 399 正确）；`sext` 按源宽度符号扩展后再按窄目标归约；
   `bitcast` 按源/目标宽度取位并按目标符号重新解释（含 F32↔i32、F64↔i64 的 reinterpret）。
3. **移位**：宽度为 32 的 `Shl/Shr/LShr/AShr` 走 i32 移位 + 按结果符号扩展；`Shr` 跟随操作数
   符号，`LShr` 显式逻辑并零扩展，`AShr` 显式算术。8/16/64 位沿用规范化的 i64 表示。
4. **f32 表示**：`sitofp→f32` 用 `f32.convert_i64_s` 并提升到 F64 局部；`fptrunc` 与 F32 算术
   结果经 `f32.demote_f64 / f64.promote_f32` 显式舍入到 binary32（W6 的值语义）；
   f32 常量在 load/store 路径仍按 4 字节编码。ABI 仍沿用既有 F64 storage（W13 单列未动）。
5. **Take**：按 `load_opcode(ins.ty)` 真发 load（含 i32 符号/零扩展与 F32 promote），与 sci
   的 move-ownership 读同义；此前为空实现。

## fail-before / pass-after（名集对照）

方法：同机同桩，before=`de72e77`，after=本笔，968 名 `rosetta/*/main.sa` 逐个跑。

| 指标 | before | after |
|---|---|---|
| 全语料 | 968 | 968 |
| rc=0 | 902 | 914 |
| 8 名输出分歧 rc=0 | 0 | 8 |
| 18 名 R3 rc=0 | 18 | 18 |
| **pass → fail** | — | **0** |

- 8 名 fail-before → pass-after：**8/8**（336, 339, 361, 362, 390, 396, 400, 402）。
- 18 名 R3 全部保持 rc=0（合并专项的回归门）。
- 簇外正当外溢 **4 名**：`345_fptrunc_f32_rounding`, `383_zext_i8_load`,
  `384_zext_i16_load`, `439_f32_round_half_even`——同根（位宽/f32 舍入），单列不稀释口径。
- 试错期曾回归的 338/343/347/399 与 404 均保持 rc=0；`pass→fail=0` 名集为空。
- 其余名集无 unexplained 翻转。

## 验证

- `cargo test --workspace`：全绿（wasm 40 项，新增 6 项：截断/扩展位宽归约、窄符号移位、
  i32→f32 reinterpret、f32 conversion、fptrunc 舍入、Take 真 load）。
- wasmtime 对 after 的全部 914 个 rc=0 模块完成校验 + 实例化 + 执行；模块结构由 wasmtime 校验器覆盖。
- JS/interp 面本笔未动。

## 后续

按 10567 口径：本专项已落位宽/符号模型本体 + 转换/Take/f32 两步。若主会话复核要求把
F32 ABI 从 F64 storage 拆成真 F32 局部（W13），另立专项；否则续推 B2 atomics 4 → R2 stub 类型 3 →
B4 UnknownRegister 1。每簇仍 fail-before/pass-after + 名集对照，pass→fail 必须为 0；10 分钟一提交推送。
