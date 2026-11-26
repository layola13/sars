pub type MaskBits = u16;

pub mod bit {
    pub const UNINITIALIZED: u16 = 0x0000;
    pub const ACTIVE: u16 = 0x0001;
    pub const LOCKED_READ: u16 = 0x0002;
    pub const LOCKED_MUT: u16 = 0x0004;
    pub const CONSUMED: u16 = 0x0008;
    pub const BORROW_VIEW: u16 = 0x0010;
    pub const FFI_BORROW: u16 = 0x0020;
    pub const UNTRACKED: u16 = 0x0040;
    pub const FALLIBLE: u16 = 0x0080;
    pub const IMMUTABLE: u16 = 0x0100;
    pub const INTERIOR_PTR: u16 = 0x0200;
    pub const OWNS_CHILDREN: u16 = 0x0400;
}

const NAMED_BITS: [(u16, &str); 11] = [
    (bit::ACTIVE, "Active"),
    (bit::LOCKED_READ, "Locked_Read"),
    (bit::LOCKED_MUT, "Locked_Mut"),
    (bit::CONSUMED, "Consumed"),
    (bit::BORROW_VIEW, "BorrowView"),
    (bit::FFI_BORROW, "FfiBorrow"),
    (bit::UNTRACKED, "Untracked"),
    (bit::FALLIBLE, "Fallible"),
    (bit::IMMUTABLE, "Immutable"),
    (bit::INTERIOR_PTR, "InteriorPtr"),
    (bit::OWNS_CHILDREN, "OwnsChildren"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapabilityMask(u16);

impl CapabilityMask {
    pub const fn new(bits: u16) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains(self, flag: u16) -> bool {
        flag != 0 && (self.0 & flag) == flag
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub const fn without(self, flag: u16) -> Self {
        Self(self.0 & !flag)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn name(self) -> &'static str {
        mask_name(self.0)
    }

    pub fn components(self) -> Vec<&'static str> {
        mask_components(self.0)
    }
}

impl From<u16> for CapabilityMask {
    fn from(bits: u16) -> Self {
        Self(bits)
    }
}

impl From<CapabilityMask> for u16 {
    fn from(mask: CapabilityMask) -> Self {
        mask.0
    }
}

pub fn mask_name(mask: u16) -> &'static str {
    match mask {
        bit::UNINITIALIZED => "Uninitialized",
        bit::ACTIVE => "Active",
        bit::LOCKED_READ => "Locked_Read",
        bit::LOCKED_MUT => "Locked_Mut",
        bit::CONSUMED => "Consumed",
        bit::BORROW_VIEW => "BorrowView",
        bit::FFI_BORROW => "FfiBorrow",
        bit::UNTRACKED => "Untracked",
        bit::FALLIBLE => "Fallible",
        bit::IMMUTABLE => "Immutable",
        bit::INTERIOR_PTR => "InteriorPtr",
        bit::OWNS_CHILDREN => "OwnsChildren",
        _ => "Composite",
    }
}

pub fn mask_components(mask: u16) -> Vec<&'static str> {
    if mask == bit::UNINITIALIZED {
        return vec!["Uninitialized"];
    }
    let mut names = Vec::new();
    let mut known = 0u16;
    for (flag, name) in NAMED_BITS {
        known |= flag;
        if mask & flag != 0 {
            names.push(name);
        }
    }
    if mask & !known != 0 {
        names.push("Unknown");
    }
    names
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Alloc,
    BorrowRead,
    BorrowMut,
    BorrowReadAgain,
    Move,
    ReleaseBorrow,
    ReleaseOwn,
    FfiRawCast,
    FfiAssumeSafe,
    FfiAssumeBorrow,
    TransferOwn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapKind {
    ReadWriteConflict,
    BorrowConflict,
    DoubleMutableBorrow,
    UseAfterMove,
    MemoryLeak,
    IllegalUnsafeContext,
    FfiOwnershipViolation,
    StackEscape,
    TransferIntoBorrow,
    TransferIntoImmutable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    pub prev_mask: u16,
    pub op: Op,
    pub legal: bool,
    pub new_mask: u16,
    pub trap: Option<TrapKind>,
}

pub const TRUTH_TABLE: [Transition; 20] = [
    Transition { prev_mask: 0x00, op: Op::Alloc, legal: true, new_mask: 0x01, trap: None },
    Transition { prev_mask: 0x01, op: Op::BorrowRead, legal: true, new_mask: 0x02, trap: None },
    Transition { prev_mask: 0x01, op: Op::BorrowMut, legal: true, new_mask: 0x04, trap: None },
    Transition { prev_mask: 0x02, op: Op::BorrowReadAgain, legal: true, new_mask: 0x02, trap: None },
    Transition { prev_mask: 0x02, op: Op::BorrowMut, legal: false, new_mask: 0x00, trap: Some(TrapKind::ReadWriteConflict) },
    Transition { prev_mask: 0x04, op: Op::BorrowRead, legal: false, new_mask: 0x00, trap: Some(TrapKind::BorrowConflict) },
    Transition { prev_mask: 0x04, op: Op::BorrowMut, legal: false, new_mask: 0x00, trap: Some(TrapKind::DoubleMutableBorrow) },
    Transition { prev_mask: 0x04, op: Op::Move, legal: false, new_mask: 0x00, trap: Some(TrapKind::BorrowConflict) },
    Transition { prev_mask: 0x01, op: Op::Move, legal: true, new_mask: 0x08, trap: None },
    Transition { prev_mask: 0x08, op: Op::Alloc, legal: false, new_mask: 0x00, trap: Some(TrapKind::UseAfterMove) },
    Transition { prev_mask: 0x11, op: Op::ReleaseBorrow, legal: true, new_mask: 0x01, trap: None },
    Transition { prev_mask: 0x01, op: Op::ReleaseOwn, legal: true, new_mask: 0x08, trap: None },
    Transition { prev_mask: 0x40, op: Op::FfiRawCast, legal: true, new_mask: 0x40, trap: None },
    Transition { prev_mask: 0x40, op: Op::FfiAssumeSafe, legal: true, new_mask: 0x01, trap: None },
    Transition { prev_mask: 0x40, op: Op::FfiAssumeBorrow, legal: true, new_mask: 0x33, trap: None },
    Transition { prev_mask: 0x100, op: Op::Alloc, legal: true, new_mask: 0x100, trap: None },
    Transition { prev_mask: 0x200, op: Op::Move, legal: true, new_mask: 0x200, trap: None },
    Transition { prev_mask: 0x01, op: Op::TransferOwn, legal: true, new_mask: 0x08, trap: None },
    Transition { prev_mask: 0x10, op: Op::TransferOwn, legal: false, new_mask: 0x00, trap: Some(TrapKind::TransferIntoBorrow) },
    Transition { prev_mask: 0x101, op: Op::TransferOwn, legal: false, new_mask: 0x00, trap: Some(TrapKind::TransferIntoImmutable) },
];

pub fn lookup(prev_mask: u16, op: Op) -> Option<Transition> {
    TRUTH_TABLE
        .iter()
        .copied()
        .find(|row| row.prev_mask == prev_mask && row.op == op)
}
