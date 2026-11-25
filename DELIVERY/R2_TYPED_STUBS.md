# R2 桩类型专项交付（回 10604）

- 裁定：10503/10604 价值序 R2 stub 类型 3（186/963-964）。不开桩冒充、不降级：
  桩实现的是 extern 契约本身（句柄、out 参数、ptr/len 对、错误码），不是给编译器打桩。
- 基线：before = 通用零值 extern 桩（旧 runner）；after = 带类型的 extern 桩 runner。
  同一编译器二进制 `e401bd1`，唯一变量是 stub 类型。只新增 `DELIVERY/` 取证件，
  未改任何编译器源码。
- 复现件：`DELIVERY/typed_stub_runner.py`（按 extern 契约实现 compress 往返、multipart
  解析/构建、sqlite prepare/step/finalize，以及 `sa_print_bytes` 打印桩）、
  `DELIVERY/r2_harness.py`、`DELIVERY/r2_fail_before_pass_after.log`。

## 根因

sars wasm 把 sa_std / sqlite 调用按 sci 同样的 `(env, name)` import 约定发射；sci 用
wasm32-wasi 静态链入 sa_std 实现这些 extern，sars 侧须由宿主提供。旧 runner 对所有
extern 一律返回零值标量：out 参数从不写入、句柄恒为 0、`sqlite3_step` 返回 0，于是
186 走错误分支，963/964 的编码/解析句柄无效。此即「桩类型」缺口，不是编译器发射缺口。

## 落地形态（完整契约，非简化）

- `sqlite3_prepare` 写有效 stmt 句柄、返回 0；`sqlite3_step` 返回 SQLITE_ROW(100)；
  `sqlite3_finalize` 返回 0 并释放句柄。
- `sa_std_compress_*`：`supported` 写 flags 位；`encode/decode` 用句柄保存字节载荷，
  `buffer_data/buffer_len` 返回同一份字节——gzip 往返按字节精确，不做有损近似。
- `sa_std_multipart_*`：按 Content-Type 的 boundary 解析固定 body，得到
  `(name, filename, content_type, body)` 四元组；builder 按同一 boundary 重建，
  再解析后 checksum 与原值一致。`part_name/part_filename/part_content_type` 写入
  调用方 buffer 并设 out_len，`part_body/buffer_data` 写 ptr/len 对。
- 宿主内存增长后再取 `data_ptr`，scratch 区放在堆/影栈之外，避免覆盖句柄槽。

## fail-before / pass-after（stdout 口径）

| 指标 | before（零值桩） | after（类型桩） |
|---|---|---|
| 全语料 | 969 | 969 |
| rc=0 | 919 | 924 |
| 3 名 R2 rc=0 | 0 | 3 |
| stdout 逐字命中 | 0 | 3 |
| pass → fail | — | 0 |

- fail-before → pass-after：**3/3**，stdout 分别 `8\n` / `ok\n` / `ok\n`。
- **不计战绩的 rc-only 假翻绿 2 名**：`301_http_client_saasm`、`302_http_server_saasm`
  在类型桩下 rc 由 2005 变 0，但 stdout 仍是 `error\n`（期望 `ok\n`），缺真实 HTTP
  extern 实现。按 10552 的输出分歧纪律，以带打印桩的 runner 复核名集后剔除，不计本簇。
- `cargo test --workspace` 全绿；编译器源码本笔未动，无 collateral 风险。

## 后续

10503/10604 价值序已走完 B2 → R2 → B4。若主会话复核要求把 R2 的 sa_std extern 实现
下沉为 wasm 内建（而非宿主类型桩），另立专项；否则本线可转下一批缺口。
