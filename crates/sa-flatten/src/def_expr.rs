use sa_ir::DefDecl;
use std::collections::HashMap;

const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefExprError {
    InvalidExpression,
    UnknownDef(String),
    RecursionLimit,
}

fn skip_spaces(text: &str, pos: &mut usize) {
    while *pos < text.len() && text.as_bytes()[*pos].is_ascii_whitespace() {
        *pos += 1;
    }
}

pub fn strip_inline_comment(expr: &str) -> &str {
    let bytes = expr.as_bytes();
    let mut in_string = false;
    let mut escape = false;
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        let c = bytes[i] as char;
        if in_string {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match c {
            '"' => in_string = true,
            '/' if bytes[i + 1] == b'/' => {
                let prev = if i == 0 { ' ' } else { bytes[i - 1] as char };
                if i == 0 || prev.is_ascii_whitespace() {
                    return &expr[..i];
                }
            }
            _ => {}
        }
        i += 1;
    }
    expr
}

fn parse_number(text: &str, pos: &mut usize) -> Option<i64> {
    let start = *pos;
    let bytes = text.as_bytes();
    let mut i = start;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let negative = bytes[start] == b'-';
    let mut radix = 10u32;
    if i + 1 < bytes.len() && bytes[i] == b'0' && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X') {
        radix = 16;
        i += 2;
    }
    let mut digits = String::new();
    while i < bytes.len() {
        let c = bytes[i] as char;
        if (radix == 16 && c.is_ascii_hexdigit()) || (radix == 10 && c.is_ascii_digit()) {
            digits.push(c);
        } else if c != '_' {
            break;
        }
        i += 1;
    }
    if digits.is_empty() {
        return None;
    }
    *pos = i;
    let unsigned = u64::from_str_radix(&digits, radix).ok()?;
    if negative {
        if unsigned == i64::MAX as u64 + 1 {
            return Some(i64::MIN);
        }
        if unsigned <= i64::MAX as u64 {
            return Some(-(unsigned as i64));
        }
        return None;
    }
    Some(unsigned as i64)
}

fn is_qualified_ident(ident: &str) -> bool {
    if ident.is_empty() || ident.starts_with('.') || ident.ends_with('.') {
        return false;
    }
    let bytes = ident.as_bytes();
    let mut segment_start = 0usize;
    for (idx, byte) in bytes.iter().enumerate() {
        if *byte != b'.' {
            continue;
        }
        if idx == segment_start || !is_ident_start(bytes[segment_start] as char) {
            return false;
        }
        if !bytes[segment_start + 1..idx]
            .iter()
            .all(|b| is_ident_continue(*b as char))
        {
            return false;
        }
        segment_start = idx + 1;
    }
    if segment_start >= bytes.len() || !is_ident_start(bytes[segment_start] as char) {
        return false;
    }
    bytes[segment_start + 1..]
        .iter()
        .all(|b| is_ident_continue(*b as char))
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn parse_identifier(text: &str, pos: &mut usize) -> Option<String> {
    let bytes = text.as_bytes();
    let start = *pos;
    if start >= bytes.len() || !is_ident_start(bytes[start] as char) {
        return None;
    }
    let mut i = start + 1;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if is_ident_continue(c) || c == '.' {
            i += 1;
        } else {
            break;
        }
    }
    let ident = &text[start..i];
    if !is_qualified_ident(ident) {
        return None;
    }
    *pos = i;
    Some(ident.to_string())
}

fn eval_with_depth(
    expr: &str,
    defs: &HashMap<String, String>,
    depth: usize,
) -> Result<i64, DefExprError> {
    let mut pos = 0usize;
    let value = parse_add_sub(expr, &mut pos, defs, depth)?;
    skip_spaces(expr, &mut pos);
    if pos != expr.len() {
        return Err(DefExprError::InvalidExpression);
    }
    Ok(value)
}

pub fn eval(expr: &str, defs: &HashMap<String, String>) -> Result<i64, DefExprError> {
    eval_with_depth(expr, defs, 0)
}

fn parse_primary(
    text: &str,
    pos: &mut usize,
    defs: &HashMap<String, String>,
    depth: usize,
) -> Result<i64, DefExprError> {
    skip_spaces(text, pos);
    if *pos >= text.len() {
        return Err(DefExprError::InvalidExpression);
    }
    if text.as_bytes()[*pos] == b'(' {
        *pos += 1;
        let value = parse_add_sub(text, pos, defs, depth + 1)?;
        skip_spaces(text, pos);
        if *pos >= text.len() || text.as_bytes()[*pos] != b')' {
            return Err(DefExprError::InvalidExpression);
        }
        *pos += 1;
        return Ok(value);
    }
    if text.as_bytes()[*pos] == b'-' {
        *pos += 1;
        return parse_primary(text, pos, defs, depth + 1).map(i64::wrapping_neg);
    }
    if let Some(number) = parse_number(text, pos) {
        return Ok(number);
    }
    if let Some(ident) = parse_identifier(text, pos) {
        if depth > MAX_DEPTH {
            return Err(DefExprError::RecursionLimit);
        }
        let value = defs
            .get(&ident)
            .ok_or_else(|| DefExprError::UnknownDef(ident.clone()))?;
        return eval_with_depth(value, defs, depth + 1);
    }
    Err(DefExprError::InvalidExpression)
}

fn parse_mul(
    text: &str,
    pos: &mut usize,
    defs: &HashMap<String, String>,
    depth: usize,
) -> Result<i64, DefExprError> {
    let mut value = parse_primary(text, pos, defs, depth)?;
    loop {
        skip_spaces(text, pos);
        if *pos >= text.len() || text.as_bytes()[*pos] != b'*' {
            break;
        }
        *pos += 1;
        let rhs = parse_primary(text, pos, defs, depth)?;
        value = value.wrapping_mul(rhs);
    }
    Ok(value)
}

fn parse_add_sub(
    text: &str,
    pos: &mut usize,
    defs: &HashMap<String, String>,
    depth: usize,
) -> Result<i64, DefExprError> {
    let mut value = parse_mul(text, pos, defs, depth)?;
    loop {
        skip_spaces(text, pos);
        if *pos >= text.len() {
            break;
        }
        let op = text.as_bytes()[*pos];
        if op != b'+' && op != b'-' {
            break;
        }
        *pos += 1;
        let rhs = parse_mul(text, pos, defs, depth)?;
        value = if op == b'+' {
            value.wrapping_add(rhs)
        } else {
            value.wrapping_sub(rhs)
        };
    }
    Ok(value)
}

pub fn normalize_defs(defs: &mut [DefDecl]) -> HashMap<String, String> {
    let mut values = HashMap::with_capacity(defs.len());
    for def in defs.iter_mut() {
        let expr = strip_inline_comment(&def.value).trim();
        match eval(expr, &values) {
            Ok(value) => {
                let normalized = value.to_string();
                def.value = normalized.clone();
                values.insert(def.name.clone(), normalized);
            }
            Err(_) => {
                values.insert(def.name.clone(), def.value.clone());
            }
        }
    }
    values
}
