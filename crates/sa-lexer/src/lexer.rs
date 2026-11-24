use crate::token::{Token, TokenKind};
use sa_ir::{Diagnostic, SourceLoc, Trap};

struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    file: &'a str,
    pos: usize,
    line: u32,
    col: u32,
    at_line_start: bool,
    out: Vec<Token>,
}

pub fn tokenize(src: &str, file: &str) -> Result<Vec<Token>, Diagnostic> {
    let mut lexer = Lexer {
        src,
        bytes: src.as_bytes(),
        file,
        pos: 0,
        line: 1,
        col: 1,
        at_line_start: true,
        out: Vec::new(),
    };
    lexer.run()
}

impl<'a> Lexer<'a> {
    fn run(&mut self) -> Result<Vec<Token>, Diagnostic> {
        while let Some(b) = self.byte(0) {
            match b {
                b' ' | b'\t' | b'\r' | 0x0b | 0x0c => {
                    self.pos += 1;
                    self.col += 1;
                }
                b'\n' => self.lex_newline(),
                b'/' if self.byte(1) == Some(b'/') => self.lex_comment(),
                b'@' => self.lex_directive(),
                b'#' => self.lex_hash_directive(),
                b'"' => self.lex_string(false)?,
                b'0'..=b'9' => self.lex_number(),
                b'A'..=b'Z' | b'a'..=b'z' | b'_' => {
                    if self.src[self.pos..].starts_with("utf8:\"") {
                        self.lex_string(true)?;
                    } else {
                        self.lex_ident();
                    }
                }
                b'-' if self.byte(1) == Some(b'>') => self.lex_double(TokenKind::Arrow),
                b'=' if self.byte(1) == Some(b'>') => self.lex_double(TokenKind::FatArrow),
                b',' => self.lex_single(TokenKind::Comma),
                b':' => self.lex_single(TokenKind::Colon),
                b'+' => self.lex_single(TokenKind::Plus),
                b'-' => self.lex_single(TokenKind::Minus),
                b'*' => self.lex_single(TokenKind::Star),
                b'&' => self.lex_single(TokenKind::Amp),
                b'^' => self.lex_single(TokenKind::Caret),
                b'!' => self.lex_single(TokenKind::Bang),
                b'?' => self.lex_single(TokenKind::Question),
                b'=' => self.lex_single(TokenKind::Equal),
                b'(' => self.lex_single(TokenKind::LParen),
                b')' => self.lex_single(TokenKind::RParen),
                b'{' => self.lex_single(TokenKind::LBrace),
                b'}' => self.lex_single(TokenKind::RBrace),
                _ => {
                    let ch = self.src[self.pos..].chars().next().unwrap();
                    let loc = self.loc();
                    self.push(TokenKind::Other(ch), ch.to_string(), loc);
                    self.pos += ch.len_utf8();
                    self.col += 1;
                }
            }
        }
        let loc = self.loc();
        self.out.push(Token::new(TokenKind::Eof, "", loc));
        Ok(std::mem::take(&mut self.out))
    }

    fn byte(&self, ahead: usize) -> Option<u8> {
        self.bytes.get(self.pos + ahead).copied()
    }

    fn loc(&self) -> SourceLoc {
        SourceLoc::new(self.file, self.line, self.col)
    }

    fn push(&mut self, kind: TokenKind, text: impl Into<String>, loc: SourceLoc) {
        self.out.push(Token::new(kind, text, loc));
    }

    fn lex_single(&mut self, kind: TokenKind) {
        let loc = self.loc();
        let text = (self.bytes[self.pos] as char).to_string();
        self.push(kind, text, loc);
        self.pos += 1;
        self.col += 1;
        self.at_line_start = false;
    }

    fn lex_double(&mut self, kind: TokenKind) {
        let loc = self.loc();
        let text = String::from_utf8_lossy(&self.bytes[self.pos..self.pos + 2]).into_owned();
        self.push(kind, text, loc);
        self.pos += 2;
        self.col += 2;
        self.at_line_start = false;
    }

    fn lex_newline(&mut self) {
        let loc = self.loc();
        self.push(TokenKind::Newline, "\n", loc);
        self.pos += 1;
        self.line += 1;
        self.col = 1;
        self.at_line_start = true;
    }

    fn lex_comment(&mut self) {
        let loc = self.loc();
        let start = self.pos;
        while let Some(b) = self.byte(0) {
            if b == b'\n' {
                break;
            }
            if b < 0x80 {
                self.pos += 1;
            } else {
                self.pos += self.src[self.pos..].chars().next().unwrap().len_utf8();
            }
        }
        let text = self.src[start..self.pos].to_string();
        self.col += text.encode_utf16().count() as u32;
        self.push(TokenKind::Comment, text, loc);
    }

    fn lex_directive(&mut self) {
        let loc = self.loc();
        let start = self.pos;
        self.pos += 1;
        self.col += 1;
        while let Some(b) = self.byte(0) {
            if is_ident_continue(b) {
                self.pos += 1;
                self.col += 1;
            } else {
                break;
            }
        }
        if self.pos == start + 1 {
            self.push(TokenKind::Other('@'), "@", loc);
        } else {
            self.push(TokenKind::Directive, self.src[start..self.pos].to_string(), loc);
        }
        self.at_line_start = false;
    }

    fn lex_hash_directive(&mut self) {
        let loc = self.loc();
        let start = self.pos;
        self.pos += 1;
        self.col += 1;
        while let Some(b) = self.byte(0) {
            if is_ident_continue(b) {
                self.pos += 1;
                self.col += 1;
            } else {
                break;
            }
        }
        if self.pos == start + 1 {
            self.push(TokenKind::Other('#'), "#", loc);
        } else {
            self.push(TokenKind::HashDirective, self.src[start..self.pos].to_string(), loc);
        }
        self.at_line_start = false;
    }

    fn lex_ident(&mut self) {
        let loc = self.loc();
        let start = self.pos;
        while let Some(b) = self.byte(0) {
            if is_ident_continue(b) {
                self.pos += 1;
                self.col += 1;
            } else {
                break;
            }
        }
        let text = self.src[start..self.pos].to_string();
        if self.at_line_start && self.byte(0) == Some(b':') {
            self.pos += 1;
            self.col += 1;
            self.push(TokenKind::Label, text, loc);
        } else {
            self.push(TokenKind::Ident, text, loc);
        }
        self.at_line_start = false;
    }

    fn lex_number(&mut self) {
        let loc = self.loc();
        let start = self.pos;
        let mut is_float = false;
        if self.byte(0) == Some(b'0') && matches!(self.byte(1), Some(b'x') | Some(b'X')) {
            self.pos += 2;
            self.col += 2;
            while let Some(b) = self.byte(0) {
                if b.is_ascii_hexdigit() || b == b'_' {
                    self.pos += 1;
                    self.col += 1;
                } else {
                    break;
                }
            }
        } else {
            while let Some(b) = self.byte(0) {
                if b.is_ascii_digit() || b == b'_' {
                    self.pos += 1;
                    self.col += 1;
                } else {
                    break;
                }
            }
            if self.byte(0) == Some(b'.') && matches!(self.byte(1), Some(b'0'..=b'9')) {
                is_float = true;
                self.pos += 1;
                self.col += 1;
                while let Some(b) = self.byte(0) {
                    if b.is_ascii_digit() || b == b'_' {
                        self.pos += 1;
                        self.col += 1;
                    } else {
                        break;
                    }
                }
            }
            if matches!(self.byte(0), Some(b'e') | Some(b'E')) {
                let mut ahead = 1;
                if matches!(self.byte(ahead), Some(b'+') | Some(b'-')) {
                    ahead += 1;
                }
                if matches!(self.byte(ahead), Some(b'0'..=b'9')) {
                    is_float = true;
                    self.pos += ahead;
                    self.col += ahead as u32;
                    while let Some(b) = self.byte(0) {
                        if b.is_ascii_digit() || b == b'_' {
                            self.pos += 1;
                            self.col += 1;
                        } else {
                            break;
                        }
                    }
                }
            }
        }
        let text = self.src[start..self.pos].to_string();
        self.push(if is_float { TokenKind::Float } else { TokenKind::Number }, text, loc);
        self.at_line_start = false;
    }

    fn lex_string(&mut self, has_prefix: bool) -> Result<(), Diagnostic> {
        let loc = self.loc();
        let start = self.pos;
        if has_prefix {
            self.pos += 5;
            self.col += 5;
        }
        self.pos += 1;
        self.col += 1;
        let mut closed = false;
        while let Some(b) = self.byte(0) {
            if b == b'\\' {
                self.pos += 1;
                self.col += 1;
                if self.byte(0).is_none() {
                    break;
                }
                let ch = self.src[self.pos..].chars().next().unwrap();
                self.pos += ch.len_utf8();
                self.col += 1;
            } else if b == b'"' {
                self.pos += 1;
                self.col += 1;
                closed = true;
                break;
            } else if b == b'\n' {
                break;
            } else {
                let ch = self.src[self.pos..].chars().next().unwrap();
                self.pos += ch.len_utf8();
                self.col += 1;
            }
        }
        if !closed {
            return Err(Diagnostic::new(
                Trap::ParseError,
                "unterminated string literal",
                loc,
            ));
        }
        let text = self.src[start..self.pos].to_string();
        self.push(TokenKind::Str, text, loc);
        self.at_line_start = false;
        Ok(())
    }
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_continue(b: u8) -> bool {
    is_ident_start(b) || b.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        tokenize(src, "t.sa").unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn directives_and_hash_directives() {
        let toks = tokenize("@import \"a.sa\"\n#def X = +0\n", "t.sa").unwrap();
        assert_eq!(toks[0].kind, TokenKind::Directive);
        assert_eq!(toks[0].text, "@import");
        assert_eq!(toks[1].kind, TokenKind::Str);
        assert_eq!(toks[3].kind, TokenKind::HashDirective);
        assert_eq!(toks[3].text, "#def");
    }

    #[test]
    fn multi_char_and_single_char_tokens() {
        assert_eq!(
            kinds("-> => , : + - * & ^ ! ? = ( ) { }"),
            vec![
                TokenKind::Arrow,
                TokenKind::FatArrow,
                TokenKind::Comma,
                TokenKind::Colon,
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Star,
                TokenKind::Amp,
                TokenKind::Caret,
                TokenKind::Bang,
                TokenKind::Question,
                TokenKind::Equal,
                TokenKind::LParen,
                TokenKind::RParen,
                TokenKind::LBrace,
                TokenKind::RBrace,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn labels_only_at_line_start() {
        let toks = tokenize("L_ENTRY:\n@main(x: i32):\n  L_NEXT:\n  y = 3\n", "t.sa").unwrap();
        assert_eq!(toks[0].kind, TokenKind::Label);
        assert_eq!(toks[0].text, "L_ENTRY");
        assert!(toks.iter().any(|t| t.kind == TokenKind::Label && t.text == "L_NEXT"));
        let colon_lines: Vec<u32> = toks.iter().filter(|t| t.kind == TokenKind::Colon).map(|t| t.loc.line).collect();
        assert!(colon_lines.contains(&2));
    }

    #[test]
    fn numbers_and_strings() {
        let toks = tokenize("0x1F 123 -4 1.5 2e3 utf8:\"a\\n\" \"b\"", "t.sa").unwrap();
        assert_eq!(toks[0].kind, TokenKind::Number);
        assert_eq!(toks[0].text, "0x1F");
        assert_eq!(toks[1].kind, TokenKind::Number);
        assert_eq!(toks[2].kind, TokenKind::Minus);
        assert_eq!(toks[3].kind, TokenKind::Number);
        assert_eq!(toks[4].kind, TokenKind::Float);
        assert_eq!(toks[5].kind, TokenKind::Float);
        assert_eq!(toks[6].kind, TokenKind::Str);
        assert_eq!(toks[6].text, "utf8:\"a\\n\"");
        assert_eq!(toks[7].kind, TokenKind::Str);
    }

    #[test]
    fn comments_newlines_and_locations() {
        let toks = tokenize("a\n// hi\n  b", "t.sa").unwrap();
        assert_eq!(toks[0].loc.line, 1);
        assert_eq!(toks[0].loc.column, 1);
        assert_eq!(toks[1].kind, TokenKind::Newline);
        assert_eq!(toks[2].kind, TokenKind::Comment);
        assert_eq!(toks[2].text, "// hi");
        let b = toks.iter().find(|t| t.is_ident("b")).unwrap();
        assert_eq!((b.loc.line, b.loc.column), (3, 3));
        assert_eq!(toks.last().unwrap().kind, TokenKind::Eof);
    }

    #[test]
    fn unknown_bytes_become_other() {
        let toks = tokenize("a @ # $", "t.sa").unwrap();
        assert_eq!(toks[1].kind, TokenKind::Other('@'));
        assert_eq!(toks[2].kind, TokenKind::Other('#'));
        assert_eq!(toks[3].kind, TokenKind::Other('$'));
    }

    #[test]
    fn unterminated_string_is_error() {
        assert!(tokenize("\"oops", "t.sa").is_err());
    }
}
