const DECLS: &str = r##"const SA_SIZE = SA_PAGES * 65536;
const memory = new ArrayBuffer(SA_SIZE);
const HEAP8 = new Int8Array(memory);
const U8 = new Uint8Array(memory);
const I32 = new Int32Array(memory);
const U32 = new Uint32Array(memory);
const F32 = new Float32Array(memory);
const F64 = new Float64Array(memory);
const DV = new DataView(memory);
const SA_SCRATCH = new DataView(new ArrayBuffer(8));
let SA_BRk = 0;
let SA_SP = SA_SIZE;
let SA_FUNCS = [];
let SA_FUNCS_BY_NAME = Object.create(null);
let SA_EXTERNS = Object.create(null);
let SA_VA = [];
let SA_VARARGS = [];
"##;

const MEM: &str = r##"function sa_num(x) {
  if (typeof x === 'bigint') return Number(x);
  if (typeof x === 'number') return x;
  if (typeof x === 'boolean') return x ? 1 : 0;
  return 0;
}
function sa_big(x) {
  if (typeof x === 'bigint') return x;
  if (typeof x === 'number') return BigInt(Math.trunc(x));
  if (typeof x === 'boolean') return BigInt(x ? 1 : 0);
  return 0n;
}
function sa_ubig(x) {
  return BigInt.asUintN(64, sa_big(x));
}
function sa_is64(a, b) {
  return typeof a === 'bigint' || typeof b === 'bigint';
}
function sa_wrap64(x) {
  return BigInt.asIntN(64, x);
}
function sa_u64(x) {
  return BigInt.asUintN(64, x);
}
function sa_ptr(x) {
  return sa_num(x);
}
function sa_addr(x, off) {
  return sa_num(x) + sa_num(off);
}
function sa_alloc(size) {
  const n = sa_num(size);
  const p = SA_BRk;
  SA_BRk = (SA_BRk + n + 7) & ~7;
  if (SA_BRk > SA_SP) sa_panic(1017, "out of memory");
  return p;
}
function sa_stack_alloc(size) {
  const n = sa_num(size);
  SA_SP = (SA_SP - n - 7) & ~7;
  if (SA_SP < SA_BRk) sa_panic(1017, "stack overflow");
  return SA_SP;
}
function sa_load(addr, ty, signed) {
  const a = sa_num(addr);
  switch (ty) {
    case 'i8': return DV.getInt8(a);
    case 'u8': return DV.getUint8(a);
    case 'i16': return DV.getInt16(a, true);
    case 'u16': return DV.getUint16(a, true);
    case 'i32': return DV.getInt32(a, true);
    case 'u32': return DV.getUint32(a, true);
    case 'f32': return DV.getFloat32(a, true);
    case 'f64': return DV.getFloat64(a, true);
    case 'i64': return DV.getBigInt64(a, true);
    case 'u64': return DV.getBigUint64(a, true);
    case 'v128': return [DV.getBigInt64(a, true), DV.getBigInt64(a + 8, true)];
    case 'i1': return signed ? DV.getInt8(a) : DV.getUint8(a);
    default: return DV.getBigInt64(a, true);
  }
}
function sa_store(addr, val, ty) {
  const a = sa_num(addr);
  switch (ty) {
    case 'i8': DV.setInt8(a, sa_num(val)); break;
    case 'u8': DV.setUint8(a, sa_num(val)); break;
    case 'i16': DV.setInt16(a, sa_num(val), true); break;
    case 'u16': DV.setUint16(a, sa_num(val), true); break;
    case 'i32': DV.setInt32(a, sa_num(val), true); break;
    case 'u32': DV.setUint32(a, sa_num(val), true); break;
    case 'f32': DV.setFloat32(a, sa_num(val), true); break;
    case 'f64': DV.setFloat64(a, sa_num(val), true); break;
    case 'i64': DV.setBigInt64(a, sa_big(val), true); break;
    case 'u64': DV.setBigUint64(a, sa_ubig(val), true); break;
    case 'v128': var v = sa_v128_of(val); DV.setBigInt64(a, v[0], true); DV.setBigInt64(a + 8, v[1], true); break;
    case 'i1': DV.setUint8(a, sa_num(val) & 1); break;
    default: DV.setBigInt64(a, sa_big(val), true); break;
  }
}
function sa_ptr_add(base, off) {
  if (typeof base === 'bigint' || typeof off === 'bigint') {
    return sa_wrap64(sa_big(base) + sa_big(off));
  }
  return sa_num(base) + sa_num(off);
}
function sa_print_bytes(ptr, len) {
  const p = sa_num(ptr), n = sa_num(len);
  const bytes = U8.subarray(p, p + n);
  if (typeof process !== 'undefined' && process.stdout && process.stdout.write && typeof Buffer !== 'undefined') {
    process.stdout.write(Buffer.from(bytes));
  } else if (typeof TextDecoder !== 'undefined') {
    console.log(new TextDecoder('utf-8').decode(bytes));
  } else {
    let s = "";
    for (let i = 0; i < n; i++) s += String.fromCharCode(U8[p + i]);
    console.log(s);
  }
  return n;
}
function sa_panic(code, msg) {
  throw new Error("SA panic " + sa_num(code) + ": " + String(msg));
}
function sa_unwrap(x) {
  if (x && typeof x === 'object' && Object.prototype.hasOwnProperty.call(x, 'ok')) {
    if (!x.ok) sa_panic(2999, "fallible error");
    return x.v;
  }
  return x;
}
function sa_release(x) {
  return x;
}
function sa_trace(f, b) {
}
"##;

const OPS: &str = r##"function sa_neg(a) {
  if (typeof a === 'bigint') return sa_wrap64(-a);
  return (-sa_num(a)) | 0;
}
function sa_add(a, b) {
  if (sa_is64(a, b)) return sa_wrap64(sa_big(a) + sa_big(b));
  return (sa_num(a) + sa_num(b)) | 0;
}
function sa_sub(a, b) {
  if (sa_is64(a, b)) return sa_wrap64(sa_big(a) - sa_big(b));
  return (sa_num(a) - sa_num(b)) | 0;
}
function sa_mul(a, b) {
  if (sa_is64(a, b)) return sa_wrap64(sa_big(a) * sa_big(b));
  return Math.imul(sa_num(a) | 0, sa_num(b) | 0);
}
function sa_sdiv(a, b) {
  if (sa_is64(a, b)) {
    const x = sa_big(a), y = sa_big(b);
    if (y === 0n) sa_panic(2003, "division by zero");
    return sa_wrap64(x / y);
  }
  const x = sa_num(a), y = sa_num(b);
  if (y === 0) sa_panic(2003, "division by zero");
  return Math.trunc(x / y) | 0;
}
function sa_udiv(a, b) {
  if (sa_is64(a, b)) {
    const y = sa_ubig(b);
    if (y === 0n) sa_panic(2003, "division by zero");
    return sa_u64(sa_ubig(a) / y);
  }
  const y = sa_num(b) >>> 0;
  if (y === 0) sa_panic(2003, "division by zero");
  return (sa_num(a) >>> 0) / y >>> 0;
}
function sa_srem(a, b) {
  if (sa_is64(a, b)) {
    const y = sa_big(b);
    if (y === 0n) sa_panic(2003, "division by zero");
    return sa_wrap64(sa_big(a) % y);
  }
  const y = sa_num(b);
  if (y === 0) sa_panic(2003, "division by zero");
  return (sa_num(a) % y) | 0;
}
function sa_urem(a, b) {
  if (sa_is64(a, b)) {
    const y = sa_ubig(b);
    if (y === 0n) sa_panic(2003, "division by zero");
    return sa_u64(sa_ubig(a) % y);
  }
  const y = sa_num(b) >>> 0;
  if (y === 0) sa_panic(2003, "division by zero");
  return (sa_num(a) >>> 0) % y >>> 0;
}
function sa_band(a, b) {
  if (sa_is64(a, b)) return sa_wrap64(sa_big(a) & sa_big(b));
  return sa_num(a) & sa_num(b);
}
function sa_bor(a, b) {
  if (sa_is64(a, b)) return sa_wrap64(sa_big(a) | sa_big(b));
  return sa_num(a) | sa_num(b);
}
function sa_bxor(a, b) {
  if (sa_is64(a, b)) return sa_wrap64(sa_big(a) ^ sa_big(b));
  return sa_num(a) ^ sa_num(b);
}
function sa_bnot(a) {
  if (typeof a === 'bigint') return sa_wrap64(~a);
  return ~sa_num(a);
}
function sa_uband(a, b) { return sa_u64(sa_ubig(a) & sa_ubig(b)); }
function sa_ubor(a, b) { return sa_u64(sa_ubig(a) | sa_ubig(b)); }
function sa_ubxor(a, b) { return sa_u64(sa_ubig(a) ^ sa_ubig(b)); }
function sa_ubnot(a) { return sa_u64(~sa_ubig(a)); }
function sa_shl(a, b) {
  if (sa_is64(a, b)) return sa_wrap64(sa_big(a) << BigInt(Number(sa_big(b) & 63n)));
  return (sa_num(a) << (sa_num(b) & 31)) | 0;
}
function sa_lshr(a, b) {
  if (sa_is64(a, b)) return sa_u64(sa_ubig(a) >> (sa_big(b) & 63n));
  return sa_num(a) >>> (sa_num(b) & 31);
}
function sa_ashr(a, b) {
  if (sa_is64(a, b)) return sa_wrap64(sa_big(a) >> (sa_big(b) & 63n));
  return sa_num(a) >> (sa_num(b) & 31);
}
function sa_eq(a, b) {
  if (sa_is64(a, b)) return sa_big(a) === sa_big(b);
  return sa_num(a) === sa_num(b);
}
function sa_ne(a, b) {
  if (sa_is64(a, b)) return sa_big(a) !== sa_big(b);
  return sa_num(a) !== sa_num(b);
}
function sa_slt(a, b) {
  if (sa_is64(a, b)) return sa_big(a) < sa_big(b);
  return sa_num(a) < sa_num(b);
}
function sa_sle(a, b) {
  if (sa_is64(a, b)) return sa_big(a) <= sa_big(b);
  return sa_num(a) <= sa_num(b);
}
function sa_sgt(a, b) {
  if (sa_is64(a, b)) return sa_big(a) > sa_big(b);
  return sa_num(a) > sa_num(b);
}
function sa_sge(a, b) {
  if (sa_is64(a, b)) return sa_big(a) >= sa_big(b);
  return sa_num(a) >= sa_num(b);
}
function sa_ult(a, b) {
  if (sa_is64(a, b)) return sa_ubig(a) < sa_ubig(b);
  return (sa_num(a) >>> 0) < (sa_num(b) >>> 0);
}
function sa_ule(a, b) {
  if (sa_is64(a, b)) return sa_ubig(a) <= sa_ubig(b);
  return (sa_num(a) >>> 0) <= (sa_num(b) >>> 0);
}
function sa_ugt(a, b) {
  if (sa_is64(a, b)) return sa_ubig(a) > sa_ubig(b);
  return (sa_num(a) >>> 0) > (sa_num(b) >>> 0);
}
function sa_uge(a, b) {
  if (sa_is64(a, b)) return sa_ubig(a) >= sa_ubig(b);
  return (sa_num(a) >>> 0) >= (sa_num(b) >>> 0);
}
function sa_div(a, b) {
  if (typeof a === 'number' && typeof b === 'number' && (a % 1 !== 0 || b % 1 !== 0)) return sa_fdiv(a, b);
  return sa_sdiv(a, b);
}
function sa_rem(a, b) {
  if (typeof a === 'number' && typeof b === 'number' && (a % 1 !== 0)) return sa_num(a) % sa_num(b);
  return sa_srem(a, b);
}
function sa_gt(a, b) {
  if (typeof a === 'number' && typeof b === 'number' && (a % 1 !== 0 || b % 1 !== 0)) return sa_fcmp_gt(a, b);
  return sa_sgt(a, b);
}
function sa_lt(a, b) {
  if (typeof a === 'number' && typeof b === 'number' && (a % 1 !== 0 || b % 1 !== 0)) return sa_fcmp_lt(a, b);
  return sa_slt(a, b);
}
function sa_shr(a, b) {
  return sa_ashr(a, b);
}
function sa_fadd(a, b) {
  return sa_num(a) + sa_num(b);
}
function sa_fsub(a, b) {
  return sa_num(a) - sa_num(b);
}
function sa_fmul(a, b) {
  return sa_num(a) * sa_num(b);
}
function sa_fdiv(a, b) {
  return sa_num(a) / sa_num(b);
}
function sa_fneg(a) {
  return -sa_num(a);
}
function sa_fcmp_eq(a, b) {
  return sa_num(a) === sa_num(b);
}
function sa_fcmp_ne(a, b) {
  return sa_num(a) !== sa_num(b);
}
function sa_fcmp_lt(a, b) {
  return sa_num(a) < sa_num(b);
}
function sa_fcmp_le(a, b) {
  return sa_num(a) <= sa_num(b);
}
function sa_fcmp_gt(a, b) {
  return sa_num(a) > sa_num(b);
}
function sa_fcmp_ge(a, b) {
  return sa_num(a) >= sa_num(b);
}
"##;

const CVT: &str = r##"function sa_cvt(kind, x, bits, signed, tofloat, fromfloat) {
  if (kind === 'fptrunc') return Math.fround(sa_num(x));
  if (kind === 'fpext') return sa_num(x);
  if (kind === 'sitofp') return Number(sa_big(x));
  if (kind === 'uitofp') {
    if (typeof x === 'bigint') return Number(BigInt.asUintN(bits, x));
    return sa_num(x) >>> 0;
  }
  if (kind === 'fptosi') {
    const z = BigInt(Math.trunc(sa_num(x)));
    if (bits > 32) return signed ? BigInt.asIntN(bits, z) : BigInt.asUintN(bits, z);
    return signed ? Number(BigInt.asIntN(bits, z)) : Number(BigInt.asUintN(bits, z));
  }
  if (kind === 'bitcast') {
    if (bits === 64) {
      if (tofloat) {
        SA_SCRATCH.setBigInt64(0, sa_big(x), true);
        return SA_SCRATCH.getFloat64(0, true);
      }
      if (!fromfloat && (typeof x === 'bigint' || Number.isInteger(sa_num(x)))) {
        return signed ? BigInt.asIntN(64, sa_big(x)) : BigInt.asUintN(64, sa_big(x));
      }
      SA_SCRATCH.setFloat64(0, sa_num(x), true);
      return SA_SCRATCH.getBigInt64(0, true);
    }
    if (tofloat) {
      SA_SCRATCH.setInt32(0, sa_num(x) | 0, true);
      return SA_SCRATCH.getFloat32(0, true);
    }
    if (!fromfloat) {
      return signed ? (sa_num(x) | 0) : (sa_num(x) >>> 0);
    }
    SA_SCRATCH.setFloat32(0, Math.fround(sa_num(x)), true);
    return SA_SCRATCH.getInt32(0, true);
  }
  const wide = bits > 32;
  if (kind === 'trunc') {
    if (typeof x === 'bigint') {
      return wide ? (signed ? BigInt.asIntN(bits, x) : BigInt.asUintN(bits, x)) : (signed ? Number(BigInt.asIntN(bits, x)) : Number(BigInt.asUintN(bits, x)));
    }
    return signed ? (sa_num(x) | 0) : (sa_num(x) >>> 0);
  }
  if (kind === 'zext') {
    if (typeof x === 'bigint') {
      return wide ? BigInt.asUintN(bits, x) : Number(BigInt.asUintN(bits, x));
    }
    if (wide) {
      const u = BigInt.asUintN(32, BigInt(sa_num(x) | 0));
      return signed ? BigInt.asIntN(bits, u) : BigInt.asUintN(bits, u);
    }
    const u = (sa_num(x) >>> 0) & ((2 ** bits) - 1);
    const half = 2 ** (bits - 1);
    return signed ? (u >= half ? u - 2 ** bits : u) : u;
  }
  if (kind === 'sext') {
    if (typeof x === 'bigint') {
      return wide ? BigInt.asIntN(bits, x) : Number(BigInt.asIntN(bits, x));
    }
    const w = BigInt(sa_num(x) | 0);
    const y = BigInt.asIntN(bits, w);
    return wide ? y : Number(y);
  }
  return x;
}
function sa_v128_of(v) {
  if (Array.isArray(v) && v.length === 2) return [sa_big(v[0]), sa_big(v[1])];
  const z = sa_big(v);
  return [z, z];
}
function sa_v128(op, a, b) {
  const x = sa_v128_of(a), y = sa_v128_of(b);
  if (op === 'add') return [sa_wrap64(x[0] + y[0]), sa_wrap64(x[1] + y[1])];
  if (op === 'sub') return [sa_wrap64(x[0] - y[0]), sa_wrap64(x[1] - y[1])];
  return [sa_wrap64(x[0] * y[0]), sa_wrap64(x[1] * y[1])];
}
function sa_extract_lane(v, i) {
  const x = sa_v128_of(v);
  return x[Number(sa_big(i)) & 1];
}
function sa_insert_lane(v, i, e) {
  const x = sa_v128_of(v);
  x[Number(sa_big(i)) & 1] = sa_big(e);
  return x;
}
function sa_shuffle_v128(a, b, m) {
  const x = sa_v128_of(a), y = sa_v128_of(b);
  const lanes = [x[0], x[1], y[0], y[1]];
  const k = sa_big(m);
  return [lanes[Number(k & 3n)], lanes[Number((k >> 2n) & 3n)]];
}
"##;

const ATOMIC: &str = r##"function sa_atomic_index(addr) {
  return (sa_num(addr) >> 2) >>> 0;
}
function sa_atomic_rmw(op, addr, val) {
  const i = sa_atomic_index(addr);
  const v = sa_num(val) | 0;
  if (op === 'add') return Atomics.add(I32, i, v);
  if (op === 'sub') return Atomics.sub(I32, i, v);
  if (op === 'and') return Atomics.and(I32, i, v);
  if (op === 'or') return Atomics.or(I32, i, v);
  if (op === 'xor') return Atomics.xor(I32, i, v);
  if (op === 'xchg') return Atomics.exchange(I32, i, v);
  for (;;) {
    const cur = Atomics.load(I32, i);
    let next = cur;
    if (op === 'umin') next = (cur >>> 0) < (v >>> 0) ? cur : v;
    else if (op === 'umax') next = (cur >>> 0) > (v >>> 0) ? cur : v;
    else if (op === 'smin') next = cur < v ? cur : v;
    else next = cur > v ? cur : v;
    if (Atomics.compareExchange(I32, i, cur, next) === cur) return cur;
  }
}
"##;

const CALL: &str = r##"function call(target, args) {
  const a = args || [];
  let f = null;
  if (typeof target === 'function') f = target;
  else if (typeof target === 'bigint' && target >= SA_FN_BASE) f = SA_FUNCS[sa_fnval(target)];
  else if (typeof target === 'number' || typeof target === 'bigint') f = SA_FUNCS[Number(target) | 0];
  else f = SA_FUNCS_BY_NAME[target] || SA_EXTERNS[target];
  if (!f) sa_panic(2002, "unknown function: " + String(target));
  SA_VARARGS = a;
  return f.apply(null, a);
}
"##;

const NPM: &str = r##"const SA_FN_BASE = 0x5341000000000000n;
function sa_fnptr(index) { return SA_FN_BASE + BigInt(index); }
function sa_fnval(value) {
  const index = BigInt(value) - SA_FN_BASE;
  return (index >= 0n && index < BigInt(SA_FUNCS.length)) ? Number(index) : -1;
}
const SA_CB_QUEUE = [];
function sa_js_make_callback(fn) {
  const index = sa_fnval(fn);
  if (index < 0) sa_panic(2002, "sa_js_make_callback: not an SA function pointer");
  const closure = (...args) => {
    const numbers = [];
    for (const value of args) {
      if (typeof value === 'number') numbers.push(value);
      else if (typeof value === 'bigint') numbers.push(Number(value));
      else if (typeof value === 'boolean') numbers.push(value ? 1 : 0);
      else sa_panic(2002, "js callback: non-numeric argument");
    }
    SA_CB_QUEUE.push({ fn: index, args: numbers });
  };
  return sa_handle_new(closure);
}
function sa_js_callback_poll(fn_out, argc_out, args_out) {
  const event = SA_CB_QUEUE.shift();
  if (!event) return 0n;
  DV.setBigUint64(sa_num(fn_out), sa_fnptr(event.fn), true);
  DV.setBigUint64(sa_num(argc_out), BigInt(event.args.length), true);
  const base = sa_num(args_out);
  for (let i = 0; i < event.args.length; i++) DV.setFloat64(base + i * 8, event.args[i], true);
  return 1n;
}
const SA_HANDLES = [null];
function sa_handle_new(value) {
  SA_HANDLES.push(value);
  return SA_HANDLES.length - 1;
}
function sa_handle_get(handle) {
  const h = sa_num(handle) | 0;
  return SA_HANDLES[h] || null;
}
function sa_js_value_release(handle) {
  const h = sa_num(handle) | 0;
  if (h > 0 && h < SA_HANDLES.length) SA_HANDLES[h] = null;
  return 0;
}
function sa_utf8(ptr, len) {
  const p = sa_num(ptr), n = sa_num(len);
  return new TextDecoder().decode(U8.subarray(p, p + n));
}
function sa_bytes_view(ptr, len) {
  const p = sa_num(ptr), n = sa_num(len);
  return U8.subarray(p, p + n);
}
"##;

pub fn prelude(pages: u32) -> String {
    let mut out = String::with_capacity(DECLS.len() + MEM.len() + OPS.len() + CVT.len() + ATOMIC.len() + CALL.len() + NPM.len() + 32);
    out.push_str(&format!("const SA_PAGES = {};\n", pages));
    out.push_str(DECLS);
    out.push_str(MEM);
    out.push_str(OPS);
    out.push_str(CVT);
    out.push_str(ATOMIC);
    out.push_str(CALL);
    out.push_str(NPM);
    out
}
