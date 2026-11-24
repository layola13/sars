pub struct Buf {
    pub s: String,
}

impl Buf {
    pub fn new() -> Self {
        Self { s: String::new() }
    }

    pub fn push_indent(&mut self, level: usize, text: &str) {
        for _ in 0..level {
            self.s.push_str("  ");
        }
        self.s.push_str(text);
        self.s.push('\n');
    }

    pub fn raw(&mut self, text: &str) {
        self.s.push_str(text);
    }
}

impl Default for Buf {
    fn default() -> Self {
        Self::new()
    }
}
