pub fn emit_shim(name: &str) -> Option<&'static str> {
    Some(match name {
        "fd_open" => r##"function fd_open(path) {
  globalThis.__sa_next_fd = (globalThis.__sa_next_fd || 3);
  return globalThis.__sa_next_fd++;
}
"##,
        "fd_close" => "function fd_close(fd) { return 0; }\n",
        "fd_read" => "function fd_read(fd) { return 3; }\n",
        "pthread_spawn" => r##"function pthread_spawn(entry, arg) {
  const i = sa_fnval(entry);
  if (i >= 0) SA_FUNCS[i](arg);
  globalThis.__sa_next_thr = (globalThis.__sa_next_thr || 1);
  return globalThis.__sa_next_thr++;
}
"##,
        "pthread_drop" => "function pthread_drop(handle) { return 0; }\n",
        "pthread_join" => "function pthread_join(handle, out) { return 0; }\n",
        "sa_pthread_join" => "function sa_pthread_join(handle, out) { return 0; }\n",
        "sqlite3_prepare" => "function sqlite3_prepare(sqlite, sql, len, stmt_out) { return 0; }\n",
        "sqlite3_step" => "function sqlite3_step(stmt) { return 100; }\n",
        "sqlite3_finalize" => "function sqlite3_finalize(stmt) { return 0; }\n",
        "sa_std_compress_supported" => r##"function sa_std_compress_supported(out_flags) {
  let flags = 0;
  let z = null;
  try { const p = globalThis.process; if (p && p.getBuiltinModule) z = p.getBuiltinModule("node:zlib"); } catch (e) {}
  if (z) { flags = 1; if (z.brotliCompressSync && z.brotliDecompressSync) flags |= 2; }
  DV.setBigUint64(sa_num(out_flags), BigInt(flags), true);
  return 0;
}
"##,
        "sa_std_compress_encode" => r##"function sa_std_compress_encode(format, data, data_len, out_handle) {
  let z = null;
  try { const p = globalThis.process; if (p && p.getBuiltinModule) z = p.getBuiltinModule("node:zlib"); } catch (e) {}
  if (!z) return 8;
  const f = sa_num(format);
  const a = sa_num(data), n = sa_num(data_len);
  const input = U8.slice(a, a + n);
  let out = null;
  try {
    if (f === 1) out = z.gzipSync(input);
    else if (f === 2) out = z.deflateSync(input);
    else if (f === 3) out = z.deflateRawSync(input);
    else if (f === 4) { if (!z.brotliCompressSync) return 8; out = z.brotliCompressSync(input); }
    else return 1;
  } catch (e) { return 6; }
  globalThis.__sa_cbuf = (globalThis.__sa_cbuf || { next: 1, map: {} });
  const id = globalThis.__sa_cbuf.next++;
  globalThis.__sa_cbuf.map[id] = { bytes: new Uint8Array(out), addr: 0 };
  DV.setBigUint64(sa_num(out_handle), BigInt(id), true);
  return 0;
}
"##,
        "sa_std_compress_decode" => r##"function sa_std_compress_decode(format, data, data_len, out_handle) {
  let z = null;
  try { const p = globalThis.process; if (p && p.getBuiltinModule) z = p.getBuiltinModule("node:zlib"); } catch (e) {}
  if (!z) return 8;
  const f = sa_num(format);
  const a = sa_num(data), n = sa_num(data_len);
  const input = U8.slice(a, a + n);
  let out = null;
  try {
    if (f === 1) out = z.gunzipSync(input);
    else if (f === 2) out = z.inflateSync(input);
    else if (f === 3) out = z.inflateRawSync(input);
    else if (f === 4) { if (!z.brotliDecompressSync) return 8; out = z.brotliDecompressSync(input); }
    else return 1;
  } catch (e) { return 6; }
  globalThis.__sa_cbuf = (globalThis.__sa_cbuf || { next: 1, map: {} });
  const id = globalThis.__sa_cbuf.next++;
  globalThis.__sa_cbuf.map[id] = { bytes: new Uint8Array(out), addr: 0 };
  DV.setBigUint64(sa_num(out_handle), BigInt(id), true);
  return 0;
}
"##,
        "sa_std_compress_buffer_data" => r##"function sa_std_compress_buffer_data(buffer) {
  const m = globalThis.__sa_cbuf;
  const rec = m ? m.map[Number(sa_num(buffer))] : null;
  if (!rec) return 0;
  if (!rec.addr) { rec.addr = sa_alloc(rec.bytes.length); U8.set(rec.bytes, rec.addr); }
  return rec.addr;
}
"##,
        "sa_std_compress_buffer_len" => r##"function sa_std_compress_buffer_len(buffer) {
  const m = globalThis.__sa_cbuf;
  const rec = m ? m.map[Number(sa_num(buffer))] : null;
  return rec ? BigInt(rec.bytes.length) : BigInt(0);
}
"##,
        "sa_std_compress_buffer_free" => r##"function sa_std_compress_buffer_free(buffer) {
  const m = globalThis.__sa_cbuf;
  if (m) { const key = Number(sa_num(buffer)); const rec = m.map[key]; if (rec) delete m.map[key]; }
  return 0;
}
"##,
        "sa_std_multipart_parse" => r##"function sa_std_multipart_parse(body, body_len, content_type, content_type_len, out_handle) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  const dec = new TextDecoder();
  const ba = sa_num(body), bn = sa_num(body_len);
  const ca = sa_num(content_type), cn = sa_num(content_type_len);
  const ct = dec.decode(U8.slice(ca, ca + cn));
  let boundary = null;
  for (const part of ct.split(";")) {
    const t = part.trim();
    if (t.startsWith("boundary=")) { boundary = t.slice(9).trim(); if (boundary.startsWith("\"") && boundary.endsWith("\"")) boundary = boundary.slice(1, -1); }
  }
  if (!boundary) return 1;
  const raw = U8.slice(ba, ba + bn);
  let text = "";
  for (let i = 0; i < raw.length; i += 8192) text += String.fromCharCode.apply(null, raw.subarray(i, i + 8192));
  const segs = text.split("--" + boundary);
  const parts = [];
  for (let i = 1; i < segs.length; i++) {
    let seg = segs[i];
    if (seg.startsWith("--")) break;
    if (!seg.startsWith("\r\n")) return 1;
    seg = seg.slice(2);
    if (seg.endsWith("\r\n")) seg = seg.slice(0, -2);
    const cut = seg.indexOf("\r\n\r\n");
    if (cut < 0) return 1;
    const head = seg.slice(0, cut);
    const pbody = seg.slice(cut + 4);
    const part = { name: "", filename: null, contentType: null, headers: [], body: new Uint8Array(pbody.length) };
    for (let j = 0; j < pbody.length; j++) part.body[j] = pbody.charCodeAt(j) & 255;
    for (const line of head.split("\r\n")) {
      const colon = line.indexOf(":");
      if (colon < 0) continue;
      const hn = line.slice(0, colon).trim();
      const hv = line.slice(colon + 1).trim();
      part.headers.push([hn, hv]);
      const lhn = hn.toLowerCase();
      if (lhn === "content-type") part.contentType = hv;
      if (lhn === "content-disposition") {
        for (const tok of hv.split(";")) {
          const t = tok.trim();
          const eq = t.indexOf("=");
          if (eq < 0) continue;
          const k = t.slice(0, eq).trim();
          let v = t.slice(eq + 1).trim();
          if (v.startsWith("\"") && v.endsWith("\"")) v = v.slice(1, -1);
          if (k === "name") part.name = v;
          if (k === "filename") part.filename = v;
        }
      }
    }
    parts.push(part);
  }
  const id = globalThis.__sa_mp.next++;
  globalThis.__sa_mp.items[id] = { kind: "parsed", parts: parts };
  DV.setBigUint64(sa_num(out_handle), BigInt(id), true);
  return 0;
}
"##,
        "sa_std_multipart_count" => r##"function sa_std_multipart_count(handle) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  const it = globalThis.__sa_mp.items[Number(sa_num(handle))];
  return (it && it.kind === "parsed") ? BigInt(it.parts.length) : BigInt(0);
}
"##,
        "sa_std_multipart_free" => r##"function sa_std_multipart_free(handle) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  delete globalThis.__sa_mp.items[Number(sa_num(handle))];
  return 0;
}
"##,
        "sa_std_multipart_part_body" => r##"function sa_std_multipart_part_body(handle, index, out_ptr, out_len) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  const it = globalThis.__sa_mp.items[Number(sa_num(handle))];
  if (!it || it.kind !== "parsed") return 3;
  const part = it.parts[sa_num(index)];
  if (!part) return 3;
  if (!part.addr) { part.addr = sa_alloc(part.body.length); U8.set(part.body, part.addr); }
  DV.setBigUint64(sa_num(out_ptr), BigInt(sa_num(part.addr)), true);
  DV.setBigUint64(sa_num(out_len), BigInt(part.body.length), true);
  return 0;
}
"##,
        "sa_std_multipart_part_name" => r##"function sa_std_multipart_part_name(handle, index, out, cap, out_len) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  const it = globalThis.__sa_mp.items[Number(sa_num(handle))];
  if (!it || it.kind !== "parsed") return 3;
  const part = it.parts[sa_num(index)];
  if (!part) return 3;
  const value = part.name;
  const enc = new TextEncoder();
  const bytes = enc.encode(value);
  DV.setBigUint64(sa_num(out_len), BigInt(bytes.length), true);
  if (sa_num(cap) < bytes.length) return 9;
  U8.set(bytes, sa_num(out));
  return 0;
}
"##,
        "sa_std_multipart_part_filename" => r##"function sa_std_multipart_part_filename(handle, index, out, cap, out_len) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  const it = globalThis.__sa_mp.items[Number(sa_num(handle))];
  if (!it || it.kind !== "parsed") return 3;
  const part = it.parts[sa_num(index)];
  if (!part) return 3;
  const value = part.filename;
  if (value === null || value === undefined) return 100;
  const enc = new TextEncoder();
  const bytes = enc.encode(value);
  DV.setBigUint64(sa_num(out_len), BigInt(bytes.length), true);
  if (sa_num(cap) < bytes.length) return 9;
  U8.set(bytes, sa_num(out));
  return 0;
}
"##,
        "sa_std_multipart_part_content_type" => r##"function sa_std_multipart_part_content_type(handle, index, out, cap, out_len) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  const it = globalThis.__sa_mp.items[Number(sa_num(handle))];
  if (!it || it.kind !== "parsed") return 3;
  const part = it.parts[sa_num(index)];
  if (!part) return 3;
  const value = part.contentType;
  if (value === null || value === undefined) return 100;
  const enc = new TextEncoder();
  const bytes = enc.encode(value);
  DV.setBigUint64(sa_num(out_len), BigInt(bytes.length), true);
  if (sa_num(cap) < bytes.length) return 9;
  U8.set(bytes, sa_num(out));
  return 0;
}
"##,
        "sa_std_multipart_part_header" => r##"function sa_std_multipart_part_header(handle, index, name, name_len, out, cap, out_len) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  const it = globalThis.__sa_mp.items[Number(sa_num(handle))];
  if (!it || it.kind !== "parsed") return 3;
  const part = it.parts[sa_num(index)];
  if (!part) return 3;
  const dec = new TextDecoder();
  const want = dec.decode(U8.slice(sa_num(name), sa_num(name) + sa_num(name_len))).toLowerCase();
  let value = null;
  for (const pair of part.headers) { if (pair[0].toLowerCase() === want) { value = pair[1]; break; } }
  if (value === null) return 100;
  const enc = new TextEncoder();
  const bytes = enc.encode(value);
  DV.setBigUint64(sa_num(out_len), BigInt(bytes.length), true);
  if (sa_num(cap) < bytes.length) return 9;
  U8.set(bytes, sa_num(out));
  return 0;
}
"##,
        "sa_std_multipart_builder_new" => r##"function sa_std_multipart_builder_new(out_handle) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  const id = globalThis.__sa_mp.next++;
  globalThis.__sa_mp.items[id] = { kind: "builder", list: [], boundary: "----sa-std-multipart-js-" + id };
  DV.setBigUint64(sa_num(out_handle), BigInt(id), true);
  return 0;
}
"##,
        "sa_std_multipart_builder_free" => r##"function sa_std_multipart_builder_free(handle) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  delete globalThis.__sa_mp.items[Number(sa_num(handle))];
  return 0;
}
"##,
        "sa_std_multipart_builder_add_field" => r##"function sa_std_multipart_builder_add_field(handle, name, name_len, value, value_len) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  const it = globalThis.__sa_mp.items[Number(sa_num(handle))];
  if (!it || it.kind !== "builder") return 2;
  const na = sa_num(name), nn = sa_num(name_len);
  const va = sa_num(value), vn = sa_num(value_len);
  it.list.push({ name: U8.slice(na, na + nn), filename: null, contentType: null, data: U8.slice(va, va + vn) });
  return 0;
}
"##,
        "sa_std_multipart_builder_add_file" => r##"function sa_std_multipart_builder_add_file(handle, name, name_len, filename, filename_len, content_type, content_type_len, data, data_len) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  const it = globalThis.__sa_mp.items[Number(sa_num(handle))];
  if (!it || it.kind !== "builder") return 2;
  it.list.push({
    name: U8.slice(sa_num(name), sa_num(name) + sa_num(name_len)),
    filename: U8.slice(sa_num(filename), sa_num(filename) + sa_num(filename_len)),
    contentType: U8.slice(sa_num(content_type), sa_num(content_type) + sa_num(content_type_len)),
    data: U8.slice(sa_num(data), sa_num(data) + sa_num(data_len))
  });
  return 0;
}
"##,
        "sa_std_multipart_builder_boundary" => r##"function sa_std_multipart_builder_boundary(handle, out, cap, out_len) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  const it = globalThis.__sa_mp.items[Number(sa_num(handle))];
  if (!it || it.kind !== "builder") return 2;
  const bytes = new TextEncoder().encode(it.boundary);
  DV.setBigUint64(sa_num(out_len), BigInt(bytes.length), true);
  if (sa_num(cap) < bytes.length) return 9;
  U8.set(bytes, sa_num(out));
  return 0;
}
"##,
        "sa_std_multipart_builder_content_type" => r##"function sa_std_multipart_builder_content_type(handle, out, cap, out_len) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  const it = globalThis.__sa_mp.items[Number(sa_num(handle))];
  if (!it || it.kind !== "builder") return 2;
  const bytes = new TextEncoder().encode("multipart/form-data; boundary=" + it.boundary);
  DV.setBigUint64(sa_num(out_len), BigInt(bytes.length), true);
  if (sa_num(cap) < bytes.length) return 9;
  U8.set(bytes, sa_num(out));
  return 0;
}
"##,
        "sa_std_multipart_builder_finish" => r##"function sa_std_multipart_builder_finish(handle, out_handle) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  const it = globalThis.__sa_mp.items[Number(sa_num(handle))];
  if (!it || it.kind !== "builder") return 2;
  const enc = new TextEncoder();
  const chunks = [];
  let first = true;
  for (const item of it.list) {
    if (!first) chunks.push(enc.encode("\r\n"));
    first = false;
    chunks.push(enc.encode("--" + it.boundary + "\r\nContent-Disposition: form-data; name=\""));
    chunks.push(item.name);
    if (item.filename !== null) {
      chunks.push(enc.encode("\"; filename=\""));
      chunks.push(item.filename);
    }
    chunks.push(enc.encode("\""));
    if (item.contentType !== null) {
      chunks.push(enc.encode("\r\nContent-Type: "));
      chunks.push(item.contentType);
    }
    chunks.push(enc.encode("\r\n\r\n"));
    chunks.push(item.data);
  }
  chunks.push(enc.encode("\r\n--" + it.boundary + "--\r\n"));
  let total = 0;
  for (const c of chunks) total += c.length;
  const bytes = new Uint8Array(total);
  let off = 0;
  for (const c of chunks) { bytes.set(c, off); off += c.length; }
  const id = globalThis.__sa_mp.next++;
  globalThis.__sa_mp.bufs[id] = { bytes: bytes, addr: 0 };
  DV.setBigUint64(sa_num(out_handle), BigInt(id), true);
  return 0;
}
"##,
        "sa_std_multipart_buffer_data" => r##"function sa_std_multipart_buffer_data(buffer) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  const rec = globalThis.__sa_mp.bufs[Number(sa_num(buffer))];
  if (!rec) return 0;
  if (!rec.addr) { rec.addr = sa_alloc(rec.bytes.length); U8.set(rec.bytes, rec.addr); }
  return rec.addr;
}
"##,
        "sa_std_multipart_buffer_len" => r##"function sa_std_multipart_buffer_len(buffer) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  const rec = globalThis.__sa_mp.bufs[Number(sa_num(buffer))];
  return rec ? BigInt(rec.bytes.length) : BigInt(0);
}
"##,
        "sa_std_multipart_buffer_free" => r##"function sa_std_multipart_buffer_free(buffer) {
  globalThis.__sa_mp = (globalThis.__sa_mp || { next: 1, items: {}, bufs: {} });
  const key = Number(sa_num(buffer));
  const rec = globalThis.__sa_mp.bufs[key];
  if (rec) { delete globalThis.__sa_mp.bufs[key]; }
  return 0;
}
"##,
        _ => return None,
    })
}
