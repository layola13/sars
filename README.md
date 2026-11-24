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

- `cargo test --workspace`: all unit tests green (Rust stable toolchain
  installed via rustup; use `export PATH=$HOME/.cargo/bin:$PATH`).
- 967/970 `sci/demos/rosetta/*/main.sa` files pass `sa check`; the remaining
  three (243, 257, 335) also fail upstream `sci`'s own `sa check`.
  `[MACRO]` / `EXPAND` bodies, multi-destination `cmpxchg`, vtable and
  `struct`/`hex` constants, `#npm_bind` directives, `@target` source regions,
  two-target `br_null`, address-based `take`, and import-relative resolution
  are implemented. The verifier models ephemeral scalars, mutable-borrow
  views and stack allocations per the Referee.
- 835/970 demos build with `build-js` and exit 0 under Node 18 (zero
  regressions vs. the previous release). `#npm_bind` externs emit ESM/CJS
  imports plus wrappers for call/ctor/method/get, UTF8 and bytes-view
  marshaling, string-out params and JS-to-SA callback queue draining;
  all ten npm rosetta demos pass end-to-end. VTable slots use the
  `SA_FN_BASE` function-pointer convention.
- hello-world, array-sum and fallible-float-loop pass end-to-end under `run`;
  `build-wasm` output validates with `WebAssembly.Module`.
- Known gaps: interpreter/JS semantics for a few macro-heavy programs
  (e.g. `104_if_let_chains` takes its error branch, `81_kv_store` hits an
  out-of-bounds read under `run`); the interpreter models fallible values as
  plain values, so `?` on a null pointer propagates early (demo 372 runs its
  error branch); JS run coverage for struct/vtable-heavy and async demos;
  native/LLVM and C backends are not ported.
