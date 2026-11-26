# DELIVERY — sla_std 薄包装双同步（Rust / sars 侧）

- 任务分支：`task/sla_std_thin_wrap_sync`（基于 `origin/main` = `8b51475`）
- 正本：`/content/sa_all/sa_plugin_sla/sla_std`（插件仓 Zig 侧，头 `2db8411a`）
- 副本：`sars/sla_std/`（本仓仓根，49 文件：顶层 2 + `wrap/` 47）
- 纪律：副本与正本逐字节一致；Rust 侧未改写任何 wrap 内容；正本问题只登记不擅改。

## 1. 复制清单（每文件 sha256，正本 / 副本对照）

统计：49/49 文件 sha256 全一致；`diff -r` 无差异。
`gen_report.json` 自述：46 模块、1260/1260 extern 全包（wrapped=1260，needs_state=702 记录在案）。

| 路径（相对 sla_std/） | 正本 sha256 | 副本 sha256 | 对照 |
|---|---|---|---|
| `sla_std/cmp.sla` | `854ce8f566558512bc05ebad777cd34f9fcce4ef0465b9f95ee8ab74bd9fbab5` | `854ce8f566558512bc05ebad777cd34f9fcce4ef0465b9f95ee8ab74bd9fbab5` | OK |
| `sla_std/std_surface.sla_meta` | `bc25862ba3e36c74e2147c36e49e0910b3f7a014e36cc7af8285970d447c0ad7` | `bc25862ba3e36c74e2147c36e49e0910b3f7a014e36cc7af8285970d447c0ad7` | OK |
| `sla_std/wrap/compress.sla` | `e0b4b583d63c36c24c99e111a3599e8c7f375e0457208820692437ac672fab8a` | `e0b4b583d63c36c24c99e111a3599e8c7f375e0457208820692437ac672fab8a` | OK |
| `sla_std/wrap/crypto.sla` | `e22659ddd5959f1c07dd4c74b843084354d59c5a233ad88be0fbb740d1c4fe49` | `e22659ddd5959f1c07dd4c74b843084354d59c5a233ad88be0fbb740d1c4fe49` | OK |
| `sla_std/wrap/dtls.sla` | `26093e167e6fe0fd4fc69ddc2d7356ceea22b88f1f45b921c8e91e34079450cc` | `26093e167e6fe0fd4fc69ddc2d7356ceea22b88f1f45b921c8e91e34079450cc` | OK |
| `sla_std/wrap/encoding/base64.sla` | `13bb6ac13662342bf19515c015b73efddab927f3eb0f81727b9617eb8b1035e8` | `13bb6ac13662342bf19515c015b73efddab927f3eb0f81727b9617eb8b1035e8` | OK |
| `sla_std/wrap/encoding/json.sla` | `64eacd8d0e5192663c982df445956d8d0005854a0dbd78633a3f5bc46bd5366a` | `64eacd8d0e5192663c982df445956d8d0005854a0dbd78633a3f5bc46bd5366a` | OK |
| `sla_std/wrap/encoding/toml.sla` | `4594b158fe508734a60577d42dd6f902b8836bc1fb4493121bca93f6bfa42b7e` | `4594b158fe508734a60577d42dd6f902b8836bc1fb4493121bca93f6bfa42b7e` | OK |
| `sla_std/wrap/encoding/yaml.sla` | `6b4e63c4065ac009bd58e27aee8b7e1f91be5073f6c38ca24fcf695f1a740109` | `6b4e63c4065ac009bd58e27aee8b7e1f91be5073f6c38ca24fcf695f1a740109` | OK |
| `sla_std/wrap/env.sla` | `660dddbd0f9cab030719b78d41167b489f57d518ea603d7c5ed296422a95dfbe` | `660dddbd0f9cab030719b78d41167b489f57d518ea603d7c5ed296422a95dfbe` | OK |
| `sla_std/wrap/fmt.sla` | `ce11937d767d9a5ce68e49b7901cf37081ae1b6a5d6632ad3d9ebfb7d4ff5f20` | `ce11937d767d9a5ce68e49b7901cf37081ae1b6a5d6632ad3d9ebfb7d4ff5f20` | OK |
| `sla_std/wrap/fs.sla` | `89e421cae398351f4b327feb0276836efc64ef4d3b375ede260dabc517c6b6f1` | `89e421cae398351f4b327feb0276836efc64ef4d3b375ede260dabc517c6b6f1` | OK |
| `sla_std/wrap/gen_report.json` | `1ba0be02b72b23a5196339eaa375b56fd0cddcd52b7b917a23f2a2ad5c70a89a` | `1ba0be02b72b23a5196339eaa375b56fd0cddcd52b7b917a23f2a2ad5c70a89a` | OK |
| `sla_std/wrap/globals.sla` | `0fd59850359dc5b341be5610be44f461415cbbf9a0d637fd79f6b9cb9202ea67` | `0fd59850359dc5b341be5610be44f461415cbbf9a0d637fd79f6b9cb9202ea67` | OK |
| `sla_std/wrap/growvec.sla` | `6067982b8e654bf42e206da257aa89173b46416c3a0170e289c71db137115055` | `6067982b8e654bf42e206da257aa89173b46416c3a0170e289c71db137115055` | OK |
| `sla_std/wrap/http2.sla` | `3378ffc8d6a036edbdc28cc40e51377994b727ff90eda1780da218574b958dc9` | `3378ffc8d6a036edbdc28cc40e51377994b727ff90eda1780da218574b958dc9` | OK |
| `sla_std/wrap/http_client.sla` | `064d5b2db91aee3430e09d355df477d14cb97d8b146716b5ff6012c46ab282df` | `064d5b2db91aee3430e09d355df477d14cb97d8b146716b5ff6012c46ab282df` | OK |
| `sla_std/wrap/http_cookie.sla` | `943f996327720d970bf057134823a64368411e9d80fa7f0ebef076ac0a5d2369` | `943f996327720d970bf057134823a64368411e9d80fa7f0ebef076ac0a5d2369` | OK |
| `sla_std/wrap/http_server.sla` | `370f8e83ef55f5a461ed68bdf051a789215fec644ef916e358ece6dfee919242` | `370f8e83ef55f5a461ed68bdf051a789215fec644ef916e358ece6dfee919242` | OK |
| `sla_std/wrap/identity.sla` | `ceb5a03c70fe66ed6da08c2926bceec4405e5d80aebb61dac55ac90366483321` | `ceb5a03c70fe66ed6da08c2926bceec4405e5d80aebb61dac55ac90366483321` | OK |
| `sla_std/wrap/io.sla` | `4785df2f99a06dd724a79b80ff6c856558b07ab26dcf5208f3700293f98ac5a3` | `4785df2f99a06dd724a79b80ff6c856558b07ab26dcf5208f3700293f98ac5a3` | OK |
| `sla_std/wrap/io/print.sla` | `8bd5d20578fb61e74f0781ac9ee234b6733e47e2d60ebf7dcf226cc895c8c86f` | `8bd5d20578fb61e74f0781ac9ee234b6733e47e2d60ebf7dcf226cc895c8c86f` | OK |
| `sla_std/wrap/math.sla` | `a8efc67cb5c723d4429b2eaff38d8c7bc2ede02596157df81eafc5a4c64686de` | `a8efc67cb5c723d4429b2eaff38d8c7bc2ede02596157df81eafc5a4c64686de` | OK |
| `sla_std/wrap/modstate.sla` | `09c34cbd2101b0e44b0e4a9fb30963975ab5fff12c25e180e98ef857835eae49` | `09c34cbd2101b0e44b0e4a9fb30963975ab5fff12c25e180e98ef857835eae49` | OK |
| `sla_std/wrap/multipart.sla` | `c1456189fb1b05bb2029bb2cac8da3cbfe4b73fa5b5cdcf9b5a0820497d2ff0d` | `c1456189fb1b05bb2029bb2cac8da3cbfe4b73fa5b5cdcf9b5a0820497d2ff0d` | OK |
| `sla_std/wrap/mysql_client.sla` | `6c04cea64b8c5b0e2a7887910b66cf3baecd8bbe91f069f43719d51317d430b8` | `6c04cea64b8c5b0e2a7887910b66cf3baecd8bbe91f069f43719d51317d430b8` | OK |
| `sla_std/wrap/net.sla` | `8c85d4d56abc1037ce521333cf8319de87edd725a6188f40d2386c552e64c881` | `8c85d4d56abc1037ce521333cf8319de87edd725a6188f40d2386c552e64c881` | OK |
| `sla_std/wrap/net_info.sla` | `2ef1a4d9618cb472acebef9d193647df0fa8a7ee340549ada406c45d00397374` | `2ef1a4d9618cb472acebef9d193647df0fa8a7ee340549ada406c45d00397374` | OK |
| `sla_std/wrap/netx.sla` | `ab7f93542f8e3c0854f7206023126a9538c2fea9833a76742862f13525749dcd` | `ab7f93542f8e3c0854f7206023126a9538c2fea9833a76742862f13525749dcd` | OK |
| `sla_std/wrap/num.sla` | `a4538bb07378659ec77cb9dbd8e33ecc105f7c47eaf35e24a0353229cf8d30c6` | `a4538bb07378659ec77cb9dbd8e33ecc105f7c47eaf35e24a0353229cf8d30c6` | OK |
| `sla_std/wrap/os/fd.sla` | `2622469ef836a7ed5b8d5ade12e562f1ca9c21f6dc906aff09c8f8705827cd8a` | `2622469ef836a7ed5b8d5ade12e562f1ca9c21f6dc906aff09c8f8705827cd8a` | OK |
| `sla_std/wrap/postgres.sla` | `34c0c6bacddd910cbd19aecd155b07d94df591941281c452971c7cc24f9ec74b` | `34c0c6bacddd910cbd19aecd155b07d94df591941281c452971c7cc24f9ec74b` | OK |
| `sla_std/wrap/process.sla` | `27f0f95f6d0fb2be1da4bf2226d58a33a47df20f77a7f362ad7fa5070e128551` | `27f0f95f6d0fb2be1da4bf2226d58a33a47df20f77a7f362ad7fa5070e128551` | OK |
| `sla_std/wrap/quic.sla` | `91b86feb650443480eb8959446f93cf751e3fecee41d2fb2e76f8124c2bd5672` | `91b86feb650443480eb8959446f93cf751e3fecee41d2fb2e76f8124c2bd5672` | OK |
| `sla_std/wrap/ssh.sla` | `d62c1807d28663a349403eb9346602c455934f618efa770372948d3c2f9e2eb8` | `d62c1807d28663a349403eb9346602c455934f618efa770372948d3c2f9e2eb8` | OK |
| `sla_std/wrap/string.sla` | `f44d3e91937293288b57bf6ca1f7fc7d944467942ad78fd946da857e372f06f1` | `f44d3e91937293288b57bf6ca1f7fc7d944467942ad78fd946da857e372f06f1` | OK |
| `sla_std/wrap/task.sla` | `42fece97b67732700ae31f973b2e285693599a4fa00b097577af83ac50e7b37d` | `42fece97b67732700ae31f973b2e285693599a4fa00b097577af83ac50e7b37d` | OK |
| `sla_std/wrap/term.sla` | `e29e251e8dba0f69e906b4ee8fe60d9015d8bce0c4d4d7a04fd35cb454bbd37f` | `e29e251e8dba0f69e906b4ee8fe60d9015d8bce0c4d4d7a04fd35cb454bbd37f` | OK |
| `sla_std/wrap/testing/assert.sla` | `5e82511ee962e4b02f33702ccb799f97687236887bbeb3544a024f890cbce46f` | `5e82511ee962e4b02f33702ccb799f97687236887bbeb3544a024f890cbce46f` | OK |
| `sla_std/wrap/text/regex.sla` | `8cbe370b52cef4db5ccb683325509770748060bd2112b746c4652055412c6779` | `8cbe370b52cef4db5ccb683325509770748060bd2112b746c4652055412c6779` | OK |
| `sla_std/wrap/thread.sla` | `285496046c9326a31df0460d2847e4d1782d111b895af00c7b69f8214a949202` | `285496046c9326a31df0460d2847e4d1782d111b895af00c7b69f8214a949202` | OK |
| `sla_std/wrap/thread_local.sla` | `7b7364d7a3590c775fb5f2fd9135dda3d7565384631ca0a103bba81599e48604` | `7b7364d7a3590c775fb5f2fd9135dda3d7565384631ca0a103bba81599e48604` | OK |
| `sla_std/wrap/time.sla` | `85f23cda9de612eb692ddab4b9f33920d0f8b270db42f1df687ed5c99832aa5c` | `85f23cda9de612eb692ddab4b9f33920d0f8b270db42f1df687ed5c99832aa5c` | OK |
| `sla_std/wrap/tls_client.sla` | `9081695d3c1873ea5349942979b6a68cc22fd16b234a1a35fa52e45b1ec7bd16` | `9081695d3c1873ea5349942979b6a68cc22fd16b234a1a35fa52e45b1ec7bd16` | OK |
| `sla_std/wrap/tls_server.sla` | `dfd838fa38ec4767d44c9baf51868381cdef85a4ee1e60fefe8c2dfb1f05b605` | `dfd838fa38ec4767d44c9baf51868381cdef85a4ee1e60fefe8c2dfb1f05b605` | OK |
| `sla_std/wrap/tree_sitter.sla` | `52683ec8fa069383724042fcdea1137e85d2fc6101a194a92aba24a209a970b6` | `52683ec8fa069383724042fcdea1137e85d2fc6101a194a92aba24a209a970b6` | OK |
| `sla_std/wrap/ts_host.sla` | `cc1a3b9a53e250ea6cd3339fd6d7fecf6950ab6c7e42a34135cdf2062b545667` | `cc1a3b9a53e250ea6cd3339fd6d7fecf6950ab6c7e42a34135cdf2062b545667` | OK |
| `sla_std/wrap/ts_unicode.sla` | `73e6c6ffaf6e136c2443d8acf67b2ff2af433cbfd10308c4079f14f72518aeb5` | `73e6c6ffaf6e136c2443d8acf67b2ff2af433cbfd10308c4079f14f72518aeb5` | OK |
| `sla_std/wrap/ws_client.sla` | `2fceed48f019142b1d18e1e51d145351c2e4b0fa9a627cb7a48f6b5ddd292925` | `2fceed48f019142b1d18e1e51d145351c2e4b0fa9a627cb7a48f6b5ddd292925` | OK |

原始清单：`DELIVERY/sla_std_source_sha256.txt`、`DELIVERY/sla_std_copy_sha256.txt`（均为 `find sla_std -type f -print0 | sort -z | xargs -0 sha256sum`）。

## 2. 放置位置与解析根的最小改动

先读了 `crates/sa-cli/src/pipeline.rs:55` 的 `add_std_roots` 与 `crates/sa-flatten/src/resolver.rs`：

- 原逻辑只把 `sa_std`（及其一级子目录）注册为解析根；`resolve_roots` 另把源文件父目录、最多 5 级祖先、cwd 注册为根，并叠加 `SA_STD`。
- 因此把 `sla_std` 放在 sars **仓根**后，`@import "sla_std/wrap/math.sla"` 能通过根/`root.join(path)` 找到；但 `is_package_path` 不认识 `sla_std`，从非顶层文件发起的同包导入不够稳健。

两处最小改动：

1. `crates/sa-cli/src/pipeline.rs`：在 `resolve_roots` 的两处 `add_std_roots` 旁各加一行 `add_std_roots(&mut roots, &current/cwd.join("sla_std"))`，把 `sla_std` 及其子目录纳入解析根，使 `sla_std/...` 成为可识别包路径。
2. 关键发现：sars parser 只接受低级 SA IR（`@import` / `@extern` / `@export NAME(...):` / 标签块 / 指令行），**无法解析 `.sla` 薄包装里的高级语法**（`fn f(&out: u64) -> i32 { return sa_f(out); }`、`struct`/`impl`）。这不是正本问题，是 Rust 侧前端能力缺口。为不在磁盘上改写 wrap 内容，新增 `crates/sa-flatten/src/sla_wrap.rs` 的**导入期内存适配层**：仅对路径含 `sla_std/wrap/` 且扩展名 `.sla` 的文件，把薄包装函数头翻译为 `@export ...:` + `L_ENTRY:`，把 `return sa_f(args);` 翻译为 `wN = call @sa_f(args); return wN`，跳过 `struct`/`impl` 块；`crates/sa-flatten/src/resolver.rs:38` 的 `read_file` 调用该适配。磁盘副本仍逐字节等于正本，适配只发生在编译器读入内容时。

## 3. 冒烟证据（完整日志见 `DELIVERY/sla_std_smoke.log`）

环境：`SA_STD=/content/sa_all/sci/sa_std:/content/sa_all/sci`（提供 wrap 传递依赖 `sa_std/*.sai`），cwd 为 sars 仓根（提供 `sla_std`）。

### 3.1 最小程序 import math wrap 并调用已包 extern — check + run 通过

源：`DELIVERY/sla_std_smoke_math.sa`
```
@import "sla_std/wrap/math.sla"
@import "sa_std/io/print.sai"
@const MSG = utf8:"sqrt wrapped ok\n"

@main() -> i32:
L_ENTRY:
    r = call @sla_math_sqrt(9.0)
    !r
    call @sa_print_bytes(&MSG, 16)
    return 0
```
```
$ ./target/release/sars check DELIVERY/sla_std_smoke_math.sa
OK: 8 functions, 18 instructions, gas used 53
$ ./target/release/sars run DELIVERY/sla_std_smoke_math.sa
sqrt wrapped ok
```

### 3.2 needs_state 模块 import + check 通过

选 `growvec`（`gen_report.json` 记 3 条 needs_state：`sa_growvec_push` / `sa_growvec_extend_from_slice` / `sa_growvec_free`）。
源：`DELIVERY/sla_std_smoke_needs_state.sa`（只 `@import "sla_std/wrap/growvec.sla"` + 空 main）
```
$ ./target/release/sars check DELIVERY/sla_std_smoke_needs_state.sa
OK: 10 functions, 18 instructions, gas used 54
```

### 3.3 回归

`cargo build --release`、`cargo test --workspace` 全绿（含新增 `sla_wrap` 3 个单测）。

## 4. 正本问题登记（不擅改）

导入期适配暴露了正本薄包装与 `sa_std/*.sai` 的 capability 标注不一致，按纪律**只登记、不修改正本或副本**：

- `compress.sla`：`sla_std_compress_encode` / `sla_std_compress_decode` 的 `data: ptr` 参数在对应 extern 上是 `&data: ptr`（borrow），sars verifier 报 `CapabilityMismatch`（E1013）。compress 虽在 needs_state 名单，但 import+check 不过；改用 growvec 完成 needs_state 冒烟。
- 多个含 struct/impl 的包装模块（如 `dtls`、`env`、`fmt`、`io`、`multipart`）在高级语法里有 `buffer: StructName` 等自定义类型，sars 的 `PrimType` 不认识；适配层跳过 struct/impl 块，故这些模块的完整语义未在 Rust 侧验证（本次任务未要求）。
- 上述均为正本/前端语义差异，未对正本或副本做任何改写。

## 5. 推送核对（KB #1/#3）

- 推前：`git ls-remote origin` → `8b51475695d23881b78baa82a816b396cef7e99b refs/heads/main`，HEAD 同值。
- 推后：见文末补记（push 后追加）。
