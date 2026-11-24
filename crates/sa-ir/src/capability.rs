use crate::types::CapPrefix;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Capability(pub u16);

impl Capability {
    pub const BY_VALUE: u16 = 0x01;
    pub const BORROW: u16 = 0x02;
    pub const MOVE: u16 = 0x04;
    pub const RAW: u16 = 0x08;
    pub const FALLIBLE: u16 = 0x80;

    pub fn new(bits: u16) -> Self {
        Self(bits)
    }

    pub fn from_prefix(cap: CapPrefix) -> Self {
        match cap {
            CapPrefix::ByValue => Self(Self::BY_VALUE),
            CapPrefix::Borrow => Self(Self::BORROW),
            CapPrefix::Move => Self(Self::MOVE),
            CapPrefix::Raw => Self(Self::RAW),
        }
    }

    pub fn has(self, bit: u16) -> bool {
        self.0 & bit != 0
    }

    pub fn add(self, bit: u16) -> Self {
        Self(self.0 | bit)
    }

    pub fn remove(self, bit: u16) -> Self {
        Self(self.0 & !bit)
    }

    pub fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub fn name(self) -> String {
        let mut parts = Vec::new();
        if self.has(Self::BY_VALUE) {
            parts.push("value");
        }
        if self.has(Self::BORROW) {
            parts.push("borrow");
        }
        if self.has(Self::MOVE) {
            parts.push("move");
        }
        if self.has(Self::RAW) {
            parts.push("raw");
        }
        if self.has(Self::FALLIBLE) {
            parts.push("fallible");
        }
        if parts.is_empty() {
            format!("0x{:02x}", self.0)
        } else {
            parts.join("|")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gas {
    pub limit: u64,
    pub used: u64,
}

impl Gas {
    pub fn new(limit: u64) -> Self {
        Self { limit, used: 0 }
    }

    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.used)
    }

    pub fn charge(&mut self, amount: u64) -> bool {
        self.used = self.used.saturating_add(amount);
        self.used <= self.limit
    }

    pub fn exceeded(&self) -> bool {
        self.used > self.limit
    }
}
