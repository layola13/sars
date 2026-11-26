pub fn runtime_source(heap_size: usize) -> String {
    format!(
        r#"/* sars native runtime shim - generated, do not edit */
#include <stdint.h>
#include <stddef.h>
#include <unistd.h>
#include <stdlib.h>

#define sa_ok_val(v) (v)
#define sa_ok_none() 0

static uint8_t sa_heap_buf[{heap}];
static size_t sa_heap_off = 0;

static void sa_panic(void) {{ _Exit(1); }}

typedef struct {{
    uint64_t fn;
    double arg;
}} sa_js_evt;
static sa_js_evt sa_js_q[32];
static uint64_t sa_js_q_head = 0;
static uint64_t sa_js_q_tail = 0;
static uint64_t sa_js_cb = 0;
static const uint8_t sa_js_user[] = "sars";

static uint8_t *sa_alloc(uint64_t bytes) {{
    size_t n = (size_t)((bytes + 15u) & ~(uint64_t)15u);
    if (n == 0) n = 16;
    if (sa_heap_off > sizeof(sa_heap_buf) ||
        n > sizeof(sa_heap_buf) - sa_heap_off) sa_panic();
    uint8_t *p = sa_heap_buf + sa_heap_off;
    sa_heap_off += n;
    return p;
}}

static void sa_print_bytes(const uint8_t *p, uint64_t len) {{
    uint64_t off = 0;
    while (off < len) {{
        ssize_t w = write(1, p + off, (size_t)(len - off));
        if (w <= 0) return;
        off += (uint64_t)w;
    }}
}}

static int64_t sa_bitcast_i64_f64(double x) {{
    union {{ uint64_t u; double d; }} v;
    v.d = x; return (int64_t)v.u;
}}
static double sa_bitcast_f64_i64(int64_t x) {{
    union {{ uint64_t u; double d; }} v;
    v.u = (uint64_t)x; return v.d;
}}
static int32_t sa_bitcast_f32_i32(float x) {{
    union {{ uint32_t u; float f; }} v;
    v.f = x; return (int32_t)v.u;
}}
static float sa_bitcast_i32_f32(int32_t x) {{
    union {{ uint32_t u; float f; }} v;
    v.u = (uint32_t)x; return v.f;
}}
"#,
        heap = heap_size
    )
}

pub fn extern_helpers_source() -> &'static str {
    r#"/* deterministic staged host bindings for C-backend externs */
#include <string.h>

typedef struct {
    uint8_t *p;
    uint64_t n;
    uint64_t off;
} sa_ext_obj;
static sa_ext_obj sa_ext_objs[64];
static uint64_t sa_ext_obj_next = 1;
static uint64_t sa_ext_fd_next = 3;
static uint64_t sa_ext_thr_next = 1;

#define SA_EXT_PTR(h) ((uint8_t *)(uintptr_t)(h))
#define SA_EXT_ID(p) ((uint64_t)(uintptr_t)(p))

static uint64_t sa_ext_obj_new(uint8_t *p, uint64_t n) {
    uint64_t id = sa_ext_obj_next++;
    if (id == 0 || id >= 64) sa_panic();
    sa_ext_objs[id].p = p;
    sa_ext_objs[id].n = n;
    sa_ext_objs[id].off = 0;
    return id;
}
static sa_ext_obj *sa_ext_obj_get(uint64_t h) {
    if (h == 0 || h >= 64) return (sa_ext_obj *)0;
    return &sa_ext_objs[(size_t)h];
}
static uint64_t sa_ext_obj_drop(uint64_t h) {
    if (h == 0 || h >= 64) return 0;
    sa_ext_objs[(size_t)h].p = (uint8_t *)0;
    sa_ext_objs[(size_t)h].n = 0;
    sa_ext_objs[(size_t)h].off = 0;
    return 0;
}

typedef struct {
    uint8_t *p;
    uint64_t n;
} sa_ext_buf;
static sa_ext_buf sa_ext_bufs[64];
static uint64_t sa_ext_buf_next = 1;

static uint64_t sa_ext_buf_new(const uint8_t *src, uint64_t n) {
    uint64_t id = sa_ext_buf_next++;
    uint8_t *p;
    if (id == 0 || id >= 64) sa_panic();
    p = sa_alloc(n == 0 ? 1 : n);
    if (n != 0) memcpy(p, src, (size_t)n);
    sa_ext_bufs[(size_t)id].p = p;
    sa_ext_bufs[(size_t)id].n = n;
    return id;
}
static sa_ext_buf *sa_ext_buf_get(uint64_t h) {
    if (h == 0 || h >= 64) return (sa_ext_buf *)0;
    return &sa_ext_bufs[(size_t)h];
}

typedef struct {
    const uint8_t *name;
    uint64_t name_len;
    const uint8_t *filename;
    uint64_t filename_len;
    const uint8_t *ctype;
    uint64_t ctype_len;
    const uint8_t *body;
    uint64_t body_len;
    const uint8_t *hdr;
    uint64_t hdr_len;
} sa_mp_part;
typedef struct {
    int kind;
    uint8_t boundary[64];
    uint64_t boundary_len;
    sa_mp_part parts[16];
    uint64_t count;
    uint8_t *data;
    uint64_t len;
} sa_mp_rec;
static sa_mp_rec sa_mps[64];
static uint64_t sa_mp_next = 1;

static uint64_t sa_mp_new(int kind) {
    uint64_t id = sa_mp_next++;
    if (id == 0 || id >= 64) sa_panic();
    memset(&sa_mps[(size_t)id], 0, sizeof(sa_mp_rec));
    sa_mps[(size_t)id].kind = kind;
    return id;
}
static sa_mp_rec *sa_mp_get(uint64_t h) {
    if (h == 0 || h >= 64) return (sa_mp_rec *)0;
    return &sa_mps[(size_t)h];
}
static int64_t sa_ext_find(const uint8_t *hay, uint64_t hn, const uint8_t *needle, uint64_t nn) {
    uint64_t i;
    if (nn == 0 || nn > hn) return -1;
    for (i = 0; i + nn <= hn; i++) {
        uint64_t j = 0;
        while (j < nn && hay[i + j] == needle[j]) j++;
        if (j == nn) return (int64_t)i;
    }
    return -1;
}
static uint64_t sa_mp_copy_out(const uint8_t *src, uint64_t len, uint8_t *out, uint64_t cap, uint8_t *out_len) {
    if (out_len != (uint8_t *)0) *(uint64_t *)(void *)out_len = len;
    if (cap < len) return 9;
    if (len != 0 && out != (uint8_t *)0) memcpy(out, src, (size_t)len);
    return 0;
}
static void sa_mp_set_boundary(sa_mp_rec *r, const uint8_t *ct, uint64_t n) {
    const uint8_t key[] = {'b','o','u','n','d','a','r','y','='};
    int64_t k = sa_ext_find(ct, n, key, sizeof(key));
    uint64_t i, s, e, len;
    if (k < 0) return;
    i = (uint64_t)k + sizeof(key);
    while (i < n && (ct[i] == ' ' || ct[i] == '\t')) i++;
    s = i;
    while (i < n && ct[i] != ';' && ct[i] != '\r' && ct[i] != '\n' && ct[i] != ' ' && ct[i] != '\t') i++;
    e = i;
    if (e > s && ct[s] == '"' && ct[e - 1] == '"') { s++; e--; }
    len = e - s;
    if (len > 63) len = 63;
    memcpy(r->boundary, ct + s, (size_t)len);
    r->boundary[(size_t)len] = 0;
    r->boundary_len = len;
}
static void sa_mp_parse_into(sa_mp_rec *r, const uint8_t *body, uint64_t n) {
    uint8_t delim[70];
    uint64_t dl = 0, pos = 0;
    const uint8_t crlf2[] = {'\r','\n','\r','\n'};
    const uint8_t nk[] = {';',' ','n','a','m','e','=','"'};
    const uint8_t fk[] = {';',' ','f','i','l','e','n','a','m','e','=','"'};
    delim[dl++] = '-'; delim[dl++] = '-';
    if (r->boundary_len != 0) {
        uint64_t i;
        for (i = 0; i < r->boundary_len; i++) delim[dl++] = r->boundary[(size_t)i];
    }
    while (pos < n && r->count < 16) {
        int64_t d = sa_ext_find(body + pos, n - pos, delim, dl);
        uint64_t seg, end;
        int64_t d2;
        if (d < 0) break;
        seg = pos + (uint64_t)d + dl;
        if (seg + 1 < n && body[(size_t)seg] == '-' && body[(size_t)(seg + 1)] == '-') break;
        if (seg + 1 < n && body[(size_t)seg] == '\r' && body[(size_t)(seg + 1)] == '\n') seg += 2;
        d2 = sa_ext_find(body + seg, n - seg, delim, dl);
        end = d2 < 0 ? n : seg + (uint64_t)d2;
        if (end >= 2 && body[(size_t)(end - 1)] == '\n' && body[(size_t)(end - 2)] == '\r') end -= 2;
        if (end > seg) {
            int64_t cut = sa_ext_find(body + seg, end - seg, crlf2, sizeof(crlf2));
            if (cut >= 0) {
                sa_mp_part *p = &r->parts[(size_t)r->count];
                uint64_t hs = seg, he = seg + (uint64_t)cut, lp;
                p->hdr = body + hs;
                p->hdr_len = he - hs;
                p->body = body + he + 4;
                p->body_len = end - (he + 4);
                lp = hs;
                while (lp < he) {
                    uint64_t le = lp, vs, ve;
                    while (le < he && !(body[(size_t)le] == '\r' && le + 1 < he && body[(size_t)(le + 1)] == '\n')) le++;
                    vs = lp;
                    while (vs < le && body[(size_t)vs] != ':') vs++;
                    if (vs < le) {
                        vs++;
                        while (vs < le && body[(size_t)vs] == ' ') vs++;
                        ve = le;
                        while (ve > vs && (body[(size_t)(ve - 1)] == ' ' || body[(size_t)(ve - 1)] == '\r')) ve--;
                        if (le - lp >= 13 && memcmp(body + lp, "Content-Type:", 13) == 0) {
                            p->ctype = body + vs;
                            p->ctype_len = ve - vs;
                        } else {
                            int64_t f = sa_ext_find(body + lp, le - lp, fk, sizeof(fk));
                            if (f >= 0) {
                                uint64_t s = lp + (uint64_t)f + sizeof(fk);
                                uint64_t e = s;
                                while (e < le && body[(size_t)e] != '"') e++;
                                p->filename = body + s;
                                p->filename_len = e - s;
                            }
                            f = sa_ext_find(body + lp, le - lp, nk, sizeof(nk));
                            if (f >= 0) {
                                uint64_t s = lp + (uint64_t)f + sizeof(nk);
                                uint64_t e = s;
                                while (e < le && body[(size_t)e] != '"') e++;
                                p->name = body + s;
                                p->name_len = e - s;
                            }
                        }
                    }
                    lp = le + 2;
                    if (lp > he) lp = he;
                }
                r->count++;
            }
        }
        pos = end;
        if (d2 < 0) break;
    }
}
static void sa_mp_finish(sa_mp_rec *r) {
    uint64_t cap = 64, i, k = 0;
    uint8_t *buf;
    const uint8_t cd[] = {'C','o','n','t','e','n','t','-','D','i','s','p','o','s','i','t','i','o','n',':',' ','f','o','r','m','-','d','a','t','a',';',' ','n','a','m','e','=','"'};
    const uint8_t fn[] = {';',' ','f','i','l','e','n','a','m','e','=','"'};
    const uint8_t ct[] = {'C','o','n','t','e','n','t','-','T','y','p','e',':'};
    const uint8_t quote[] = {'"'};
    const uint8_t crlf[] = {'\r','\n'};
    const uint8_t blank[] = {'\r','\n'};
    for (i = 0; i < r->count; i++) {
        sa_mp_part *p = &r->parts[(size_t)i];
        cap += r->boundary_len + 160 + p->name_len + p->filename_len + p->ctype_len + p->body_len;
    }
    buf = sa_alloc(cap);
    for (i = 0; i < r->count; i++) {
        sa_mp_part *p = &r->parts[(size_t)i];
        uint64_t j;
        buf[k++] = '-'; buf[k++] = '-';
        for (j = 0; j < r->boundary_len; j++) buf[k++] = r->boundary[(size_t)j];
        memcpy(buf + k, cd, sizeof(cd)); k += sizeof(cd);
        if (p->name_len != 0) { memcpy(buf + k, p->name, (size_t)p->name_len); k += p->name_len; }
        if (p->filename_len != 0) {
            memcpy(buf + k, fn, sizeof(fn)); k += sizeof(fn);
            memcpy(buf + k, p->filename, (size_t)p->filename_len); k += p->filename_len;
        }
        memcpy(buf + k, quote, sizeof(quote)); k += sizeof(quote);
        memcpy(buf + k, crlf, sizeof(crlf)); k += sizeof(crlf);
        if (p->ctype_len != 0) {
            uint64_t j2;
            memcpy(buf + k, ct, sizeof(ct)); k += sizeof(ct);
            for (j2 = 0; j2 < p->ctype_len; j2++) buf[k++] = p->ctype[(size_t)j2];
            buf[k++] = '\r'; buf[k++] = '\n';
        }
        memcpy(buf + k, blank, sizeof(blank)); k += sizeof(blank);
        if (p->body_len != 0) { memcpy(buf + k, p->body, (size_t)p->body_len); k += p->body_len; }
        buf[k++] = '\r'; buf[k++] = '\n';
    }
    buf[k++] = '-'; buf[k++] = '-';
    for (i = 0; i < r->boundary_len; i++) buf[k++] = r->boundary[(size_t)i];
    buf[k++] = '\r'; buf[k++] = '\n';
    r->data = buf;
    r->len = k;
}
static uint64_t sa_ext_buf_drop(uint64_t h) {
    sa_ext_buf *b = sa_ext_buf_get(h);
    if (b == (sa_ext_buf *)0) return 0;
    b->p = (uint8_t *)0;
    b->n = 0;
    return 0;
}
static uint64_t sa_mp_drop(uint64_t h) {
    sa_mp_rec *r = sa_mp_get(h);
    if (r == (sa_mp_rec *)0) return 0;
    r->kind = 0;
    return 0;
}
static uint64_t sa_mp_header_value(sa_mp_part *p, const uint8_t *name, uint64_t nlen, const uint8_t **out, uint64_t *out_len) {
    uint64_t i;
    if (p == (sa_mp_part *)0 || nlen == 0) return 100;
    for (i = 0; i + nlen <= p->hdr_len; i++) {
        uint64_t v, e;
        if (memcmp(p->hdr + i, name, (size_t)nlen) != 0) continue;
        v = i + nlen;
        if (v >= p->hdr_len || p->hdr[v] != ':') continue;
        v++;
        while (v < p->hdr_len && p->hdr[v] == ' ') v++;
        e = v;
        while (e < p->hdr_len && p->hdr[e] != '\r' && p->hdr[e] != '\n') e++;
        *out = p->hdr + v;
        *out_len = e - v;
        return 0;
    }
    return 100;
}
static uint64_t sa_mp_content_type(sa_mp_rec *r, uint8_t *out, uint64_t cap, uint8_t *out_len) {
    const uint8_t pre[] = {'m','u','l','t','i','p','a','r','t','/','f','o','r','m','-','d','a','t','a',';',' ','b','o','u','n','d','a','r','y','='};
    uint64_t len = sizeof(pre) + r->boundary_len;
    if (out_len != (uint8_t *)0) *(uint64_t *)(void *)out_len = len;
    if (cap < len) return 9;
    if (out == (uint8_t *)0) return 0;
    memcpy(out, pre, sizeof(pre));
    memcpy(out + sizeof(pre), r->boundary, (size_t)r->boundary_len);
    return 0;
}
static uint8_t sa_ext_path[] = {'/', 's', 't', 'r', 'e', 'a', 'm'};
static uint8_t sa_ext_hdr[] = {'t', 'e', 'x', 't', '/', 'p', 'l', 'a', 'i', 'n'};
"#
}
