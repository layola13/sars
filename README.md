# sars — SA compiler in Rust

`sars` is a clean-room Rust reimplementation of the `sci` compiler core
(SA safe IR + Referee verifier + backends). It is a Cargo **crates workspace**;
every Rust source file stays below 1000 lines.

## Crates

| crate | role |
|---|---|
| `sa-ir` | shared IR: types, instructions, program, traps, capabilities |
| `sa-lexer` | `.sa` / `.sai` / `.sal` tokenizer with source locations |
| `sa-parser` | tokens -> IR program |
| `sa-flatten` | `@import` expansion, `#def` macros, forbidden-syntax gate |
| `sa-verify` | Referee: affine/borrow/phi/capability verifier |
| `sa-emit-js` | self-contained ESM/CJS JavaScript emitter |
| `sa-emit-wasm` | WASI-oriented WASM binary emitter |
| `sa-interp` | register interpreter for `sa run` |
| `sa-cli` | `sa` command line driver |

## Quick start

```bash
cargo build --release
./target/release/sa check  ../sci/demos/rosetta/13_array_sum/main.sa
./target/release/sa run    ../sci/demos/rosetta/13_array_sum/main.sa
./target/release/sa build-js ../sci/demos/rosetta/01_hello_world/main.sa -o hello.mjs
```

## Current status

- `cargo test --workspace`: all unit tests green.
- 120/120 sampled `sci/demos/rosetta/*/main.sa` files pass `sa check`;
  the full 968-file corpus reaches 932 passes with no regressions.
  `[MACRO]` / `EXPAND` bodies, multi-destination `cmpxchg`, vtable constants
  and import-relative resolution are implemented.
- hello-world, array-sum and fallible-float-loop pass end-to-end under `run`
  and `build-js` (verified with Node); `build-wasm` output validates with
  `WebAssembly.Module`.
- Known gaps: interpreter/JS semantics for a few macro-heavy programs
  (e.g. `104_if_let_chains` takes its error branch) and some
  vtable/struct-heavy demos; native/LLVM and C backends are not ported.
