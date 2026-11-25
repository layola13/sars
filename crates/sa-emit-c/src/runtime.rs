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
"#,
        heap = heap_size
    )
}
