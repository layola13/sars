use crate::leb::{put_bytes, put_sleb, put_uleb, section};

pub fn functype(params: &[u8], results: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(0x60);
    put_uleb(&mut out, params.len() as u64);
    out.extend_from_slice(params);
    put_uleb(&mut out, results.len() as u64);
    out.extend_from_slice(results);
    out
}

pub fn vec_of(items: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    put_uleb(&mut out, items.len() as u64);
    for item in items {
        out.extend_from_slice(item);
    }
    out
}

pub fn import_entry(module: &str, name: &str, type_index: u32) -> Vec<u8> {
    let mut out = Vec::new();
    put_bytes(&mut out, module.as_bytes());
    put_bytes(&mut out, name.as_bytes());
    out.push(0x00);
    put_uleb(&mut out, type_index as u64);
    out
}

pub fn export_entry(name: &str, kind: u8, index: u32) -> Vec<u8> {
    let mut out = Vec::new();
    put_bytes(&mut out, name.as_bytes());
    out.push(kind);
    put_uleb(&mut out, index as u64);
    out
}

pub fn u32_vec(items: &[u32]) -> Vec<u8> {
    let mut out = Vec::new();
    put_uleb(&mut out, items.len() as u64);
    for v in items {
        put_uleb(&mut out, *v as u64);
    }
    out
}

pub fn locals_vec(locals: &[(u32, u8)]) -> Vec<u8> {
    let mut out = Vec::new();
    put_uleb(&mut out, locals.len() as u64);
    for (count, ty) in locals {
        put_uleb(&mut out, *count as u64);
        out.push(*ty);
    }
    out
}

pub fn type_section(types: &[Vec<u8>]) -> Vec<u8> {
    section(1, &vec_of(types))
}

pub fn import_section(imports: &[Vec<u8>]) -> Vec<u8> {
    section(2, &vec_of(imports))
}

pub fn function_section(indices: &[u32]) -> Vec<u8> {
    section(3, &u32_vec(indices))
}

pub fn memory_section(pages: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    put_uleb(&mut payload, 1);
    payload.push(0x00);
    put_uleb(&mut payload, pages as u64);
    section(5, &payload)
}

pub fn table_section(min: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    put_uleb(&mut payload, 1);
    payload.push(0x70);
    payload.push(0x00);
    put_uleb(&mut payload, min as u64);
    section(4, &payload)
}

pub fn element_section(funcs: &[u32]) -> Vec<u8> {
    let mut payload = Vec::new();
    put_uleb(&mut payload, 1);
    put_uleb(&mut payload, 0);
    payload.push(0x41);
    put_sleb(&mut payload, 0);
    payload.push(0x0b);
    put_uleb(&mut payload, funcs.len() as u64);
    for &index in funcs {
        put_uleb(&mut payload, index as u64);
    }
    section(9, &payload)
}

pub fn export_section(exports: &[Vec<u8>]) -> Vec<u8> {
    section(7, &vec_of(exports))
}

pub fn code_section(bodies: &[Vec<u8>]) -> Vec<u8> {
    let mut payload = Vec::new();
    put_uleb(&mut payload, bodies.len() as u64);
    for body in bodies {
        put_uleb(&mut payload, body.len() as u64);
        payload.extend_from_slice(body);
    }
    section(10, &payload)
}

pub fn data_section(segments: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut payload = Vec::new();
    put_uleb(&mut payload, segments.len() as u64);
    for (offset, bytes) in segments {
        put_uleb(&mut payload, 0);
        payload.push(0x41);
        put_sleb(&mut payload, *offset as i32 as i64);
        payload.push(0x0b);
        put_bytes(&mut payload, bytes);
    }
    section(11, &payload)
}

pub fn name_section(entries: &[(u32, String)]) -> Vec<u8> {
    let mut payload = Vec::new();
    put_bytes(&mut payload, b"name");
    let mut subsection = Vec::new();
    put_uleb(&mut subsection, entries.len() as u64);
    for (index, name) in entries {
        put_uleb(&mut subsection, *index as u64);
        put_bytes(&mut subsection, name.as_bytes());
    }
    payload.push(0x01);
    put_uleb(&mut payload, subsection.len() as u64);
    payload.extend_from_slice(&subsection);
    section(0, &payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_type_encoding() {
        assert_eq!(functype(&[0x7f], &[0x7e]), vec![0x60, 1, 0x7f, 1, 0x7e]);
    }

    #[test]
    fn memory_section_encoding() {
        let s = memory_section(2);
        assert_eq!(s, vec![5, 3, 1, 0, 2]);
    }

    #[test]
    fn name_section_has_correct_id() {
        let s = name_section(&[(0, "main".to_string())]);
        assert_eq!(s[0], 0);
        assert!(s[1] > 0);
    }
}
