use crate::parser::{join, split_commas, string_value, take_type_suffix};
use sa_ir::{
    CapPrefix, ConstDecl, ConstValue, DefDecl, FunctionKind, ImportDecl, LocDecl, NpmBindDecl,
    NpmBindShape, ParamSpec, PrimType, Signature,
};
use sa_lexer::{Token, TokenKind};

pub(crate) fn parse_import(line: &[Token]) -> Option<ImportDecl> {
    let path = line.get(1)?;
    if path.kind != TokenKind::Str {
        return None;
    }
    Some(ImportDecl::new(string_value(&path.text), path.loc.clone()))
}

pub(crate) fn parse_npm_bind(line: &[Token]) -> Option<NpmBindDecl> {
    let name = line.get(1)?;
    if name.kind != TokenKind::Ident {
        return None;
    }
    let raw = line.get(2)?;
    if raw.kind != TokenKind::Str {
        return None;
    }
    let binding = string_value(&raw.text);
    let hash = binding.find('#')?;
    let spec = binding[..hash].to_string();
    if spec.is_empty() {
        return None;
    }
    let rest = &binding[hash + 1..];
    let (target, keywords) = match rest.split_once(':') {
        Some((target, keywords)) => (target, keywords),
        None => (rest, ""),
    };
    if target.is_empty() {
        return None;
    }
    let mut shape = NpmBindShape::Call;
    for keyword in keywords.split(':').filter(|kw| !kw.is_empty()) {
        match keyword {
            "ctor" => shape = NpmBindShape::Ctor,
            "method" => shape = NpmBindShape::Method,
            "get" => shape = NpmBindShape::Get,
            "bytes" | "f32view" => {}
            _ => return None,
        }
    }
    if matches!(shape, NpmBindShape::Method | NpmBindShape::Get) && spec != "@handle" {
        return None;
    }
    Some(NpmBindDecl {
        extern_name: name.text.clone(),
        spec,
        target: target.to_string(),
        shape,
        loc: line[0].loc.clone(),
    })
}

pub(crate) fn parse_def(line: &[Token]) -> Option<DefDecl> {
    let name = line.get(1)?;
    if name.kind != TokenKind::Ident {
        return None;
    }
    if line.get(2).map(|t| t.kind) != Some(TokenKind::Equal) {
        return None;
    }
    let value = join(&line[3..]);
    if value.is_empty() {
        return None;
    }
    Some(DefDecl {
        name: name.text.clone(),
        value,
        loc: line[0].loc.clone(),
    })
}

pub(crate) fn parse_loc(line: &[Token]) -> Option<LocDecl> {
    let file = line.get(1)?;
    if file.kind != TokenKind::Str {
        return None;
    }
    if line.get(2).map(|t| t.kind) != Some(TokenKind::Colon) {
        return None;
    }
    let line_no = line.get(3)?;
    if line_no.kind != TokenKind::Number {
        return None;
    }
    if line.get(4).map(|t| t.kind) != Some(TokenKind::Colon) {
        return None;
    }
    let col = line.get(5)?;
    if col.kind != TokenKind::Number {
        return None;
    }
    let n = |s: &str| s.replace('_', "").parse::<u32>().ok();
    Some(LocDecl {
        file: string_value(&file.text),
        line: n(&line_no.text)?,
        column: n(&col.text)?,
        original_line: n(&line_no.text)?,
        loc: line[0].loc.clone(),
    })
}

pub(crate) fn parse_signature(
    line: &[Token],
    kind: FunctionKind,
) -> Result<Signature, String> {
    let (name, paren) = match line.get(1) {
        Some(t) if t.kind == TokenKind::Ident || t.kind == TokenKind::Str => (t.text.clone(), 2),
        _ if line[0].text.len() > 1 => (line[0].text.trim_start_matches('@').to_string(), 1),
        _ => return Err("missing function name".into()),
    };
    if line.get(paren).map(|t| t.kind) != Some(TokenKind::LParen) {
        return Err("expected '(' in function header".into());
    }
    let mut i = paren + 1;
    let mut params = Vec::new();
    let mut is_variadic = false;
    while i < line.len() && line[i].kind != TokenKind::RParen {
        if is_ellipsis(line, i) {
            is_variadic = true;
            i += 3;
            if i < line.len() && line[i].kind == TokenKind::Comma {
                i += 1;
            }
            continue;
        }
        let cap = match line[i].kind {
            TokenKind::Amp => {
                i += 1;
                CapPrefix::Borrow
            }
            TokenKind::Caret => {
                i += 1;
                CapPrefix::Move
            }
            TokenKind::Star => {
                i += 1;
                CapPrefix::Raw
            }
            _ => CapPrefix::ByValue,
        };
        let pname = match line.get(i) {
            Some(t) if t.kind == TokenKind::Ident => t.text.clone(),
            _ => return Err("missing parameter name".into()),
        };
        i += 1;
        let ty = if line.get(i).map(|t| t.kind) == Some(TokenKind::Colon) {
            i += 1;
            match line.get(i).and_then(|t| PrimType::parse(&t.text)) {
                Some(t) => {
                    i += 1;
                    t
                }
                None => return Err(format!("unknown parameter type for {pname}")),
            }
        } else {
            PrimType::Ptr
        };
        params.push(ParamSpec::new(pname, ty, cap));
        if i < line.len() && line[i].kind == TokenKind::Comma {
            i += 1;
        } else if i < line.len() && line[i].kind != TokenKind::RParen {
            return Err("expected ',' or ')' in parameter list".into());
        }
    }
    if line.get(i).map(|t| t.kind) != Some(TokenKind::RParen) {
        return Err("missing ')' in function header".into());
    }
    i += 1;
    let mut return_ty = PrimType::Void;
    let mut return_cap = None;
    let mut return_fallible = false;
    if line.get(i).map(|t| t.kind) == Some(TokenKind::Arrow) {
        i += 1;
        return_cap = match line.get(i).map(|t| t.kind) {
            Some(TokenKind::Caret) => {
                i += 1;
                Some(CapPrefix::Move)
            }
            Some(TokenKind::Amp) => {
                i += 1;
                Some(CapPrefix::Borrow)
            }
            Some(TokenKind::Star) => {
                i += 1;
                Some(CapPrefix::Raw)
            }
            _ => None,
        };
        return_ty = match line.get(i).and_then(|t| PrimType::parse(&t.text)) {
            Some(t) => {
                i += 1;
                t
            }
            None => return Err("unknown return type".into()),
        };
        if line.get(i).map(|t| t.kind) == Some(TokenKind::Bang) {
            return_fallible = true;
            i += 1;
        }
    }
    if kind != FunctionKind::External {
        if line.get(i).map(|t| t.kind) != Some(TokenKind::Colon) {
            return Err("function header must end with ':'".into());
        }
        i += 1;
        if i != line.len() {
            return Err("unexpected tokens after function header".into());
        }
    }
    let mut sig = Signature::new(name);
    sig.params = params;
    sig.kind = kind;
    sig.return_cap = return_cap;
    sig.return_ty = return_ty;
    sig.return_fallible = return_fallible;
    sig.is_variadic = is_variadic;
    Ok(sig)
}

fn is_ellipsis(line: &[Token], i: usize) -> bool {
    (0..3).all(|k| line.get(i + k).map(|t| t.kind) == Some(TokenKind::Other('.')))
}

pub(crate) fn parse_const(line: &[Token]) -> Result<ConstDecl, String> {
    let name = match line.get(1) {
        Some(t) if t.kind == TokenKind::Ident => t.text.clone(),
        _ => return Err("missing constant name".into()),
    };
    if line.get(2).map(|t| t.kind) != Some(TokenKind::Equal) {
        return Err("expected '=' in @const declaration".into());
    }
    let mut rest: Vec<Token> = line[3..].to_vec();
    let ty = take_type_suffix(&mut rest);
    let value = const_value(&rest).ok_or_else(|| "invalid constant value".to_string())?;
    let mut decl = ConstDecl::new(name, value);
    decl.ty = ty;
    Ok(decl)
}

fn const_value(tokens: &[Token]) -> Option<ConstValue> {
    if tokens.is_empty() {
        return None;
    }
    if tokens[0].is_ident("struct")
        && tokens.get(1).map(|t| t.kind) == Some(TokenKind::LBrace)
        && tokens.last().map(|t| t.kind) == Some(TokenKind::RBrace)
    {
        let mut fields = Vec::new();
        for part in split_commas(&tokens[2..tokens.len() - 1]) {
            let eq = part.iter().position(|t| t.kind == TokenKind::Equal)?;
            let mut bytes = match const_value(&part[eq + 1..])? {
                ConstValue::Bytes(b) => b,
                ConstValue::Utf8(s) => s.into_bytes(),
                ConstValue::Int(v) => v.to_le_bytes().to_vec(),
                _ => return None,
            };
            if let Some(size_token) = part[..eq]
                .iter()
                .rev()
                .find(|t| t.kind == TokenKind::Number)
            {
                if let Ok(size) = size_token.text.parse::<usize>() {
                    bytes.resize(size, 0);
                    bytes.truncate(size);
                }
            }
            fields.push(ConstValue::Bytes(bytes));
        }
        return Some(ConstValue::Aggregate(fields));
    }
    if tokens[0].is_ident("hex") && tokens.get(1).map(|t| t.kind) == Some(TokenKind::Colon) {
        let raw: String = tokens[2..].iter().map(|t| t.text.as_str()).collect();
        let src = raw.as_bytes();
        let mut bytes = Vec::new();
        let mut i = 0usize;
        while i < src.len() {
            if src[i] == b'\\' && i + 3 < src.len() && src[i + 1] == b'x' {
                if let Ok(byte) = u8::from_str_radix(&raw[i + 2..i + 4], 16) {
                    bytes.push(byte);
                    i += 4;
                    continue;
                }
            }
            bytes.push(src[i]);
            i += 1;
        }
        return Some(ConstValue::Bytes(bytes));
    }
    if tokens[0].is_ident("vtable")
        && tokens.get(1).map(|t| t.kind) == Some(TokenKind::LBrace)
        && tokens.last().map(|t| t.kind) == Some(TokenKind::RBrace)
    {
        let mut entries = Vec::new();
        for part in split_commas(&tokens[2..tokens.len() - 1]) {
            if part.len() >= 3 && part[1].kind == TokenKind::Equal {
                let slot = part[0].text.clone();
                let target = join(&part[2..]);
                entries.push(ConstValue::Utf8(format!("{slot}:{target}")));
            }
        }
        return Some(ConstValue::VTable(entries));
    }
    if tokens[0].is_ident("agg")
        && tokens.get(1).map(|t| t.kind) == Some(TokenKind::Colon)
        && tokens.get(2).map(|t| t.kind) == Some(TokenKind::Other('['))
        && tokens.last().map(|t| t.kind) == Some(TokenKind::Other(']'))
    {
        let inner = &tokens[3..tokens.len() - 1];
        let mut values = Vec::new();
        for part in split_commas(inner) {
            values.push(const_value(&part)?);
        }
        return Some(ConstValue::Aggregate(values));
    }
    if tokens[0].is_ident("bytes")
        && tokens.get(1).map(|t| t.kind) == Some(TokenKind::Colon)
    {
        let bytes = if tokens.get(2).map(|t| t.kind) == Some(TokenKind::Str) {
            string_value(&tokens[2].text).into_bytes()
        } else {
            join(&tokens[2..]).into_bytes()
        };
        return Some(ConstValue::Bytes(bytes));
    }
    if tokens.len() == 1 && tokens[0].kind == TokenKind::Ident {
        match tokens[0].text.as_str() {
            "true" => return Some(ConstValue::Bool(true)),
            "false" => return Some(ConstValue::Bool(false)),
            "null" => return Some(ConstValue::Null),
            "undef" => return Some(ConstValue::Undef),
            _ => {}
        }
    }
    let text = join(tokens);
    match sa_ir::Operand::parse(&text) {
        sa_ir::Operand::Int(v) => Some(ConstValue::Int(v)),
        sa_ir::Operand::Float(v) => Some(ConstValue::Float(v)),
        sa_ir::Operand::Utf8(s) => Some(ConstValue::Utf8(s)),
        sa_ir::Operand::Bytes(b) => Some(ConstValue::Bytes(b)),
        sa_ir::Operand::Null => Some(ConstValue::Null),
        sa_ir::Operand::Undef => Some(ConstValue::Undef),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_source;

    fn tokens(src: &str) -> Vec<Token> {
        sa_lexer::tokenize(src, "t.sa")
            .unwrap()
            .into_iter()
            .filter(|t| t.kind != TokenKind::Eof)
            .collect()
    }

    #[test]
    fn signature_with_caps_variadic_and_fallible_return() {
        let line = tokens("@ffi_wrapper f(&a: i32, ^b: ptr, *c: blob_handle, ...) -> ^ptr!:");
        let sig = parse_signature(&line, FunctionKind::FfiWrapper).unwrap();
        assert_eq!(sig.name, "f");
        assert_eq!(sig.kind, FunctionKind::FfiWrapper);
        assert_eq!(sig.params.len(), 3);
        assert_eq!(sig.params[0].cap, CapPrefix::Borrow);
        assert_eq!(sig.params[1].cap, CapPrefix::Move);
        assert_eq!(sig.params[2].cap, CapPrefix::Raw);
        assert!(sig.is_variadic);
        assert_eq!(sig.return_cap, Some(CapPrefix::Move));
        assert_eq!(sig.return_ty, PrimType::Ptr);
        assert!(sig.return_fallible);
    }

    #[test]
    fn extern_signature_needs_no_colon() {
        let line = tokens("@extern fd_open(*path: ptr) -> i32");
        let sig = parse_signature(&line, FunctionKind::External).unwrap();
        assert_eq!(sig.kind, FunctionKind::External);
        assert_eq!(sig.return_ty, PrimType::I32);
    }

    #[test]
    fn const_value_forms() {
        for (src, expected) in [
            ("@const A = 0xFF", ConstValue::Int(255)),
            ("@const A = -2", ConstValue::Int(-2)),
            ("@const A = 1.5 as f64", ConstValue::Float(1.5)),
            ("@const A = true", ConstValue::Bool(true)),
            ("@const A = null", ConstValue::Null),
            ("@const A = undef", ConstValue::Undef),
            ("@const A = utf8:\"x\\n\"", ConstValue::Utf8("x\n".into())),
            ("@const A = bytes:\"ab\"", ConstValue::Bytes(b"ab".to_vec())),
        ] {
            let line = tokens(src);
            let c = parse_const(&line).unwrap();
            assert_eq!(c.value, expected, "{src}");
        }
        let line = tokens("@const A = agg:[1, 2, utf8:\"z\"] as i32");
        let c = parse_const(&line).unwrap();
        assert_eq!(
            c.value,
            ConstValue::Aggregate(vec![
                ConstValue::Int(1),
                ConstValue::Int(2),
                ConstValue::Utf8("z".into()),
            ])
        );
        assert_eq!(c.ty, Some(PrimType::I32));
    }

    #[test]
    fn def_and_loc() {
        let d = parse_def(&tokens("#def Point_x = +4")).unwrap();
        assert_eq!((d.name.as_str(), d.value.as_str()), ("Point_x", "+4"));
        let l = parse_loc(&tokens("#loc \"gen.sa\":12:3")).unwrap();
        assert_eq!(l.file, "gen.sa");
        assert_eq!((l.line, l.column), (12, 3));
        assert_eq!(l.original_line, 12);
    }

    #[test]
    fn headers_in_program() {
        let p = parse_source(
            "@export e() -> void:\nL_ENTRY:\n  nop\n@test \"case one\"():\nL_ENTRY:\n  nop\n@gpu g():\nL_ENTRY:\n  nop\n",
            "t.sa",
        )
        .unwrap();
        assert_eq!(p.functions[0].sig.kind, FunctionKind::Exported);
        assert_eq!(p.functions[1].sig.kind, FunctionKind::TestFunc);
        assert_eq!(p.functions[2].sig.kind, FunctionKind::Gpu);
    }
}
