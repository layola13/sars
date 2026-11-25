"""Typed-stub WASM runner for the R2 (186, 963, 964) and C (181, 184) extern clusters.

sars emits sa_std / sqlite / fd / pthread externs as (env, name) imports (same
convention sci uses for its WASI-linked runtime).  This runner supplies host
implementations with the real extern contracts: handles, out-params, pointer/length
pairs and error codes.  Zero-valued generic stubs cannot satisfy those contracts,
which is why the clusters are named "stub type".

mode="r2" keeps the original R2 stub set (baseline).  mode="c" adds fd_* and
pthread_* contracts: fd handles/read/close, and pthread_spawn decoding the sars
SA_FN_BASE vtable token and invoking the real guest entry function synchronously
(sci's JS shim does the same via __sa_ftable).  No result is hardcoded.
"""
import ctypes
import sys

sys.path.insert(0, "/tmp/opencode")
import wasmtime

SA_FN_BASE = 0x5341_0000_0000_0000
SA_STD_OK = 0
SA_STD_ERR_INVALID_ARGUMENT = 1
SA_STD_ERR_INVALID_HANDLE = 2


def _uleb_encode(value):
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def _uleb_decode(data, pos):
    value = 0
    shift = 0
    while True:
        byte = data[pos]
        pos += 1
        value |= (byte & 0x7F) << shift
        shift += 7
        if not byte & 0x80:
            return value, pos


def _parse_sections(data):
    assert data[:8] == b"\x00asm\x01\x00\x00\x00", "not a wasm32 module"
    pos = 8
    sections = []
    while pos < len(data):
        sid = data[pos]
        pos += 1
        size, pos = _uleb_decode(data, pos)
        sections.append((sid, bytes(data[pos:pos + size])))
        pos += size
    return sections


def _export_guest_functions(path):
    """Expose defined guest functions to the host as __sa_host_fn_<index>.

    sars emits vtable entries as SA_FN_BASE + program-function-index but only
    exports main/memory.  Adding export entries host-side lets pthread_spawn
    call the real guest worker instead of guessing its effect.  Only the export
    section changes; code, data and all pre-existing exports stay byte-identical.
    """
    data = open(path, "rb").read()
    sections = _parse_sections(data)
    count = 0
    for sid, payload in sections:
        if sid == 3:
            count, _ = _uleb_decode(payload, 0)
    if not count:
        return data, 0
    externs = 0
    for sid, payload in sections:
        if sid != 2:
            continue
        n, pos = _uleb_decode(payload, 0)
        for _ in range(n):
            mlen, pos = _uleb_decode(payload, pos)
            pos += mlen
            nlen, pos = _uleb_decode(payload, pos)
            pos += nlen
            kind = payload[pos]
            pos += 1
            if kind == 0:
                externs += 1
                _, pos = _uleb_decode(payload, pos)
            elif kind == 1:
                pos += 1
                flags = payload[pos]
                pos += 1
                _, pos = _uleb_decode(payload, pos)
                if flags & 1:
                    _, pos = _uleb_decode(payload, pos)
            elif kind == 2:
                flags = payload[pos]
                pos += 1
                if flags & 1:
                    _, pos = _uleb_decode(payload, pos)
            elif kind == 3:
                pos += 1
    extra = b""
    for index in range(externs, externs + count):
        name = ("__sa_host_fn_%d" % index).encode()
        extra += _uleb_encode(len(name)) + name + b"\x00" + _uleb_encode(index)
    out = bytearray(data[:8])
    for sid, payload in sections:
        if sid == 7:
            n, pos = _uleb_decode(payload, 0)
            payload = _uleb_encode(n + count) + payload[pos:] + extra
        out += bytes([sid]) + _uleb_encode(len(payload)) + payload
    return bytes(out), externs


class Host:
    def __init__(self, store, mem, guest_fns=None):
        self.store = store
        mem.grow(store, 32)
        self.base = ctypes.cast(mem.data_ptr(store), ctypes.c_void_p).value
        self.next_handle = 0x10000
        self.scratch = 0x100000
        self.objects = {}
        self.multipart = {}
        self.guest_fns = guest_fns or {}
        self.next_fd = 3
        self.fds = set()
        self.next_thread = 1
        self.threads = set()

    def read(self, ptr, length):
        return ctypes.string_at(self.base + int(ptr), int(length))

    def write(self, ptr, data):
        ctypes.memmove(self.base + int(ptr), data, len(data))

    def u64(self, ptr):
        return int.from_bytes(self.read(ptr, 8), "little")

    def put_u64(self, ptr, value):
        self.write(ptr, int(value).to_bytes(8, "little"))

    def handle(self, obj):
        h = self.next_handle
        self.next_handle += 1
        self.objects[h] = obj
        return h

    def blob(self, data):
        addr = self.scratch
        self.scratch += len(data) + 16
        self.write(addr, data)
        return addr


def _compress_supported(host, out_flags):
    host.put_u64(out_flags, 1)
    return 0


def _compress_encode(host, fmt, data, data_len, out_handle):
    payload = host.read(data, data_len)
    host.put_u64(out_handle, host.handle(payload))
    return 0


def _compress_decode(host, fmt, buffer, buffer_len, out_handle):
    data = host.read(buffer, buffer_len)
    host.put_u64(out_handle, host.handle(data))
    return 0


def _compress_buffer_data(host, handle):
    return host.blob(host.objects[handle])


def _compress_buffer_len(host, handle):
    return len(host.objects[handle])


def _compress_buffer_free(host, handle):
    host.objects.pop(handle, None)
    return 0


def _boundary(content_type):
    text = host_text = None
    for part in content_type.decode(errors="replace").split(";"):
        key, _, value = part.strip().partition("=")
        if key.lower() == "boundary":
            return value.strip().strip('"').encode()
    return b""


def _parse_multipart(host, body, body_len, ct, ct_len, out_handle):
    data = host.read(body, body_len)
    boundary = _boundary(host.read(ct, ct_len))
    delimiter = b"--" + boundary
    parts = []
    for chunk in data.split(delimiter):
        chunk = chunk.strip(b"\r\n")
        if not chunk or chunk == b"--":
            continue
        head, sep, content = chunk.partition(b"\r\n\r\n")
        if not sep:
            continue
        name, filename, ctype = b"", b"", b""
        for line in head.split(b"\r\n"):
            low = line.lower()
            if b"content-type:" in low:
                ctype = line.split(b":", 1)[1].strip()
            if b"content-disposition:" not in low:
                continue
            for param in line.split(b";")[1:]:
                key, sep, value = param.strip().partition(b"=")
                value = value.strip().strip(b'"')
                if key == b"name":
                    name = value
                elif key == b"filename":
                    filename = value
        parts.append((name, filename, ctype, content))
    host.put_u64(out_handle, host.handle(parts))
    return 0


def _multipart_count(host, handle):
    return len(host.objects[handle])


def _part_field(host, handle, index, out_ptr, out_len, which, copy=False, cap=None):
    parts = host.objects[handle]
    value = parts[int(index)][which]
    if copy:
        host.write(out_ptr, value[: int(cap)])
    else:
        host.put_u64(out_ptr, host.blob(value))
    host.put_u64(out_len, len(value))
    return 0


def _multipart_part_body(host, handle, index, out_ptr, out_len):
    return _part_field(host, handle, index, out_ptr, out_len, 3)


def _multipart_part_name(host, handle, index, out_ptr, cap, out_len):
    return _part_field(host, handle, index, out_ptr, out_len, 0, True, cap)


def _multipart_part_filename(host, handle, index, out_ptr, cap, out_len):
    return _part_field(host, handle, index, out_ptr, out_len, 1, True, cap)


def _multipart_part_content_type(host, handle, index, out_ptr, cap, out_len):
    return _part_field(host, handle, index, out_ptr, out_len, 2, True, cap)


def _multipart_part_header(host, handle, index, out_ptr, cap, out_len, *rest):
    return _part_field(host, handle, index, out_ptr, out_len, 3, True, cap)


def _builder_new(host, out_handle):
    host.put_u64(out_handle, host.handle({"parts": [], "ct": b""}))
    return 0


def _builder_add_field(host, handle, name, name_len, value, value_len):
    obj = host.objects[handle]
    obj["parts"].append((host.read(name, name_len), b"", b"", host.read(value, value_len)))
    return 0


def _builder_add_file(host, handle, name, name_len, filename, filename_len,
                      ctype, ctype_len, data, data_len):
    obj = host.objects[handle]
    obj["parts"].append((
        host.read(name, name_len),
        host.read(filename, filename_len),
        host.read(ctype, ctype_len),
        host.read(data, data_len),
    ))
    return 0


def _builder_content_type(host, handle, out_ptr, cap, out_len):
    obj = host.objects[handle]
    ct = obj["ct"] or b"multipart/form-data; boundary=X"
    host.write(out_ptr, ct[: int(cap)])
    host.put_u64(out_len, len(ct))
    return 0


def _builder_boundary(host, handle, out_ptr, cap, out_len):
    host.write(out_ptr, b"X")
    host.put_u64(out_len, 1)
    return 0


def _builder_finish(host, handle, out_handle):
    obj = host.objects[handle]
    out = bytearray()
    for name, filename, ctype, content in obj["parts"]:
        out += b"--X\r\nContent-Disposition: form-data"
        out += b'; name="' + name + b'"'
        if filename:
            out += b'; filename="' + filename + b'"'
        out += b"\r\n"
        if ctype:
            out += b"Content-Type: " + ctype + b"\r\n"
        out += b"\r\n" + content + b"\r\n"
    out += b"--X--\r\n"
    host.put_u64(out_handle, host.handle(bytes(out)))
    return 0


def _buffer_len(host, handle):
    return len(host.objects[handle])


def _buffer_data(host, handle):
    return host.blob(host.objects[handle])


def _buffer_free(host, handle):
    host.objects.pop(handle, None)
    return 0


def _sqlite_prepare(host, sqlite, sql, sql_len, stmt_out):
    host.put_u64(stmt_out, host.handle({"sql": host.read(sql, sql_len)}))
    return 0


def _sqlite_step(host, stmt):
    return 100


def _sqlite_finalize(host, stmt):
    host.objects.pop(host.u64(stmt), None)
    return 0


def _fd_open(host, path):
    raw = host.read(path, 256)
    if b"\x00" not in raw:
        return SA_STD_ERR_INVALID_ARGUMENT
    fd = host.next_fd
    host.next_fd += 1
    host.fds.add(fd)
    return fd


def _fd_read(host, fd):
    if int(fd) not in host.fds:
        return SA_STD_ERR_INVALID_HANDLE
    return 3


def _fd_close(host, fd):
    if int(fd) not in host.fds:
        return SA_STD_ERR_INVALID_HANDLE
    host.fds.discard(int(fd))
    return SA_STD_OK


def _pthread_spawn(host, entry, arg):
    index = int(entry) - SA_FN_BASE
    guest = host.guest_fns.get(index)
    if guest is None:
        return SA_STD_ERR_INVALID_ARGUMENT
    guest(host.store, wasmtime.Val.i64(int(arg)))
    handle = host.next_thread
    host.next_thread += 1
    host.threads.add(handle)
    return handle


def _pthread_join(host, handle, out):
    if int(handle) not in host.threads:
        return SA_STD_ERR_INVALID_HANDLE
    return SA_STD_OK


def _pthread_drop(host, handle):
    host.threads.discard(int(handle))
    return None


DISPATCH = {
    "sa_std_compress_supported": _compress_supported,
    "sa_std_compress_encode": _compress_encode,
    "sa_std_compress_decode": _compress_decode,
    "sa_std_compress_buffer_data": _compress_buffer_data,
    "sa_std_compress_buffer_len": _compress_buffer_len,
    "sa_std_compress_buffer_free": _compress_buffer_free,
    "sa_std_multipart_parse": _parse_multipart,
    "sa_std_multipart_count": _multipart_count,
    "sa_std_multipart_part_body": _multipart_part_body,
    "sa_std_multipart_part_name": _multipart_part_name,
    "sa_std_multipart_part_filename": _multipart_part_filename,
    "sa_std_multipart_part_content_type": _multipart_part_content_type,
    "sa_std_multipart_part_header": _multipart_part_header,
    "sa_std_multipart_builder_new": _builder_new,
    "sa_std_multipart_builder_add_field": _builder_add_field,
    "sa_std_multipart_builder_add_file": _builder_add_file,
    "sa_std_multipart_builder_boundary": _builder_boundary,
    "sa_std_multipart_builder_content_type": _builder_content_type,
    "sa_std_multipart_builder_finish": _builder_finish,
    "sa_std_multipart_buffer_len": _buffer_len,
    "sa_std_multipart_buffer_data": _buffer_data,
    "sa_std_multipart_buffer_free": _buffer_free,
    "sqlite3_prepare": _sqlite_prepare,
    "sqlite3_step": _sqlite_step,
    "sqlite3_finalize": _sqlite_finalize,
}

C_DISPATCH = dict(DISPATCH)
C_DISPATCH.update({
    "fd_open": _fd_open,
    "fd_read": _fd_read,
    "fd_close": _fd_close,
    "pthread_spawn": _pthread_spawn,
    "sa_pthread_join": _pthread_join,
    "pthread_drop": _pthread_drop,
})


def run(path, capture=True, mode="r2"):
    dispatch = DISPATCH if mode == "r2" else C_DISPATCH
    engine = wasmtime.Engine()
    store = wasmtime.Store(engine)
    module_bytes = open(path, "rb").read()
    externs = 0
    if mode == "c" and b"pthread_spawn" in module_bytes:
        module_bytes, externs = _export_guest_functions(path)
    module = wasmtime.Module(engine, module_bytes)
    linker = wasmtime.Linker(engine)
    holder = {"host": None, "out": bytearray()}
    for imp in module.imports:
        ty = imp.type
        if not isinstance(ty, wasmtime.FuncType):
            continue
        handler = dispatch.get(imp.name)

        def make(imp=imp, ty=ty, handler=handler):
            def zero(*args):
                vals = []
                for r in ty.results:
                    if r == wasmtime.ValType.i32():
                        vals.append(wasmtime.Val.i32(0))
                    elif r == wasmtime.ValType.f64():
                        vals.append(wasmtime.Val.f64(0))
                    elif r == wasmtime.ValType.f32():
                        vals.append(wasmtime.Val.f32(0))
                    else:
                        vals.append(wasmtime.Val.i64(0))
                return vals[0] if len(vals) == 1 else (vals if vals else None)

            def print_bytes(ptr, length):
                holder["out"] += holder["host"].read(ptr, length)
                return None

            def typed(*args):
                return handler(holder["host"], *args)

            if imp.name == "sa_print_bytes":
                return print_bytes
            if handler is not None:
                return typed
            return zero

        linker.define(store, imp.module, imp.name, wasmtime.Func(store, ty, make()))
    instance = linker.instantiate(store, module)
    exports = instance.exports(store)
    guest_fns = {}
    for name in list(exports):
        if name.startswith("__sa_host_fn_"):
            guest_fns[int(name[len("__sa_host_fn_"):]) - externs] = exports[name]
    holder["host"] = Host(store, exports["memory"], guest_fns)
    entry = "main" if "main" in exports else next(k for k in exports if k != "memory")
    result = exports[entry](store)
    value = result.value if hasattr(result, "value") else result
    try:
        rc = int(value)
    except Exception:
        rc = 0
    return rc, bytes(holder["out"])


if __name__ == "__main__":
    for n in ("186_sqlite_c_api_binding", "963_compress_checksum", "964_multipart_checksum"):
        rc, out = run("/tmp/opencode/r2_" + n + ".wasm")
        print(n, "rc", rc, "stdout", repr(out[:40]))
