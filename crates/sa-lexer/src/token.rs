use sa_ir::SourceLoc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Ident,
    Number,
    Float,
    Str,
    Directive,
    HashDirective,
    Label,
    Comma,
    Colon,
    Arrow,
    FatArrow,
    Plus,
    Minus,
    Star,
    Amp,
    Caret,
    Bang,
    Question,
    Equal,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Newline,
    Comment,
    Other(char),
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub loc: SourceLoc,
}

impl Token {
    pub fn new(kind: TokenKind, text: impl Into<String>, loc: SourceLoc) -> Self {
        Self {
            kind,
            text: text.into(),
            loc,
        }
    }

    pub fn is(&self, kind: TokenKind) -> bool {
        self.kind == kind
    }

    pub fn is_ident(&self, text: &str) -> bool {
        self.kind == TokenKind::Ident && self.text == text
    }
}
