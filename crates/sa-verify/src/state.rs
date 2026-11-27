use crate::mask::bit;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegKind {
    Alloc,
    StackAlloc,
    Value,
    Scalar,
    Param,
    Ref,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegState {
    Undefined,
    Owned { kind: RegKind },
    Moved,
    Borrowed { shared: usize, mutable: bool },
    Released,
    Conflict,
}

pub type RegMap = HashMap<String, RegState>;

#[derive(Debug, Clone, Default)]
pub struct FnState {
    pub regs: RegMap,
    pub masks: HashMap<String, u16>,
    pub fallible: HashMap<String, bool>,
    pub borrow_origin: HashMap<String, String>,
    pub origin_base: HashMap<String, RegState>,
    pub escaped: HashSet<String>,
    pub borrow_params: HashSet<String>,
}

impl FnState {
    pub fn state_of(&self, reg: &str) -> RegState {
        self.regs.get(reg).cloned().unwrap_or(RegState::Undefined)
    }

    pub fn set(&mut self, reg: &str, state: RegState) {
        self.regs.insert(reg.to_string(), state);
    }

    pub fn mask_of(&self, reg: &str) -> u16 {
        if let Some(mask) = self.masks.get(reg) {
            return *mask;
        }
        match self.state_of(reg) {
            RegState::Undefined | RegState::Conflict => bit::UNINITIALIZED,
            RegState::Owned { kind: RegKind::Scalar } => bit::UNTRACKED,
            RegState::Owned { .. } => bit::ACTIVE,
            RegState::Moved | RegState::Released => bit::CONSUMED,
            RegState::Borrowed { shared, mutable } => {
                if self.borrow_origin.contains_key(reg) {
                    bit::ACTIVE | bit::BORROW_VIEW
                } else if mutable {
                    bit::LOCKED_MUT
                } else if shared > 0 {
                    bit::LOCKED_READ
                } else {
                    bit::ACTIVE | bit::BORROW_VIEW
                }
            }
        }
    }

    pub fn set_mask(&mut self, reg: &str, mask: u16) {
        self.masks.insert(reg.to_string(), mask);
    }

    pub fn is_same(&self, other: &FnState) -> bool {
        self.regs == other.regs
            && self.masks == other.masks
            && self.fallible == other.fallible
            && self.borrow_origin == other.borrow_origin
            && self.origin_base == other.origin_base
            && self.escaped == other.escaped
            && self.borrow_params == other.borrow_params
    }

    pub fn merge(&self, other: &FnState) -> (FnState, Vec<(String, RegState, RegState)>) {
        let mut out = FnState::default();
        let mut conflicts = Vec::new();
        let mut keys: Vec<&String> = self.regs.keys().chain(other.regs.keys()).collect();
        keys.sort();
        keys.dedup();
        for key in keys {
            let a = self.state_of(key);
            let b = other.state_of(key);
            let merged = merge_reg(&a, &b);
            if merged == RegState::Conflict
                && a != b
                && a != RegState::Undefined
                && b != RegState::Undefined
            {
                conflicts.push((key.clone(), a, b));
            }
            if merged != RegState::Undefined {
                out.regs.insert(key.clone(), merged);
            }
        }
        let mut mask_keys: Vec<&String> = self.masks.keys().chain(other.masks.keys()).collect();
        mask_keys.sort();
        mask_keys.dedup();
        for key in mask_keys {
            let left = self.masks.get(key).copied().unwrap_or_else(|| self.mask_of(key));
            let right = other.masks.get(key).copied().unwrap_or_else(|| other.mask_of(key));
            let joined = crate::merge::join_masks(left, right).0;
            if joined != bit::UNINITIALIZED {
                out.masks.insert(key.clone(), joined);
            }
        }
        for (key, value) in &self.fallible {
            if other.fallible.get(key) == Some(value) {
                out.fallible.insert(key.clone(), *value);
            }
        }
        for (key, value) in &self.borrow_origin {
            if other.borrow_origin.get(key) == Some(value) {
                out.borrow_origin.insert(key.clone(), value.clone());
            }
        }
        for (key, value) in &self.origin_base {
            if other.origin_base.get(key) == Some(value) {
                out.origin_base.insert(key.clone(), value.clone());
            }
        }
        out.escaped = self.escaped.union(&other.escaped).cloned().collect();
        out.borrow_params = self.borrow_params.union(&other.borrow_params).cloned().collect();
        (out, conflicts)
    }
}

pub(crate) fn merge_reg(a: &RegState, b: &RegState) -> RegState {
    if a == b {
        return a.clone();
    }
    if *a == RegState::Undefined {
        return b.clone();
    }
    if *b == RegState::Undefined {
        return a.clone();
    }
    if let (
        RegState::Borrowed { shared: sa, mutable: ma },
        RegState::Borrowed { shared: sb, mutable: mb },
    ) = (a, b)
    {
        return RegState::Borrowed {
            shared: (*sa).max(*sb),
            mutable: *ma || *mb,
        };
    }
    RegState::Conflict
}

pub trait RegStateBackend: Clone + Default {
    fn state_of(&self, reg: &str) -> RegState;
    fn set(&mut self, reg: &str, state: RegState);
    fn mask_of(&self, reg: &str) -> u16;
    fn set_mask(&mut self, reg: &str, mask: u16);
    fn fallible_get(&self, reg: &str) -> Option<bool>;
    fn fallible_insert(&mut self, reg: &str, value: bool);
    fn borrow_origin_contains(&self, reg: &str) -> bool;
    fn borrow_origin_insert(&mut self, dst: &str, origin: &str);
    fn borrow_origin_remove(&mut self, reg: &str) -> Option<String>;
    fn origin_base_entry_or_insert(&mut self, reg: &str, state: RegState);
    fn origin_base_get(&self, reg: &str) -> Option<RegState>;
    fn origin_base_remove(&mut self, reg: &str) -> Option<RegState>;
    fn escaped_contains(&self, reg: &str) -> bool;
    fn escaped_insert(&mut self, reg: &str);
    fn borrow_params_contains(&self, reg: &str) -> bool;
    fn borrow_params_insert(&mut self, reg: &str);
    fn regs_snapshot(&self) -> Vec<(String, RegState)>;
    fn is_same(&self, other: &Self) -> bool;
    fn merge(&self, other: &Self) -> (Self, Vec<(String, RegState, RegState)>);
}

impl RegStateBackend for FnState {
    fn state_of(&self, reg: &str) -> RegState {
        FnState::state_of(self, reg)
    }

    fn set(&mut self, reg: &str, state: RegState) {
        FnState::set(self, reg, state);
    }

    fn mask_of(&self, reg: &str) -> u16 {
        FnState::mask_of(self, reg)
    }

    fn set_mask(&mut self, reg: &str, mask: u16) {
        FnState::set_mask(self, reg, mask);
    }

    fn fallible_get(&self, reg: &str) -> Option<bool> {
        self.fallible.get(reg).copied()
    }

    fn fallible_insert(&mut self, reg: &str, value: bool) {
        self.fallible.insert(reg.to_string(), value);
    }

    fn borrow_origin_contains(&self, reg: &str) -> bool {
        self.borrow_origin.contains_key(reg)
    }

    fn borrow_origin_insert(&mut self, dst: &str, origin: &str) {
        self.borrow_origin.insert(dst.to_string(), origin.to_string());
    }

    fn borrow_origin_remove(&mut self, reg: &str) -> Option<String> {
        self.borrow_origin.remove(reg)
    }

    fn origin_base_entry_or_insert(&mut self, reg: &str, state: RegState) {
        self.origin_base.entry(reg.to_string()).or_insert(state);
    }

    fn origin_base_get(&self, reg: &str) -> Option<RegState> {
        self.origin_base.get(reg).cloned()
    }

    fn origin_base_remove(&mut self, reg: &str) -> Option<RegState> {
        self.origin_base.remove(reg)
    }

    fn escaped_contains(&self, reg: &str) -> bool {
        self.escaped.contains(reg)
    }

    fn escaped_insert(&mut self, reg: &str) {
        self.escaped.insert(reg.to_string());
    }

    fn borrow_params_contains(&self, reg: &str) -> bool {
        self.borrow_params.contains(reg)
    }

    fn borrow_params_insert(&mut self, reg: &str) {
        self.borrow_params.insert(reg.to_string());
    }

    fn regs_snapshot(&self) -> Vec<(String, RegState)> {
        self.regs
            .iter()
            .map(|(name, state)| (name.clone(), state.clone()))
            .collect()
    }

    fn is_same(&self, other: &Self) -> bool {
        FnState::is_same(self, other)
    }

    fn merge(&self, other: &Self) -> (Self, Vec<(String, RegState, RegState)>) {
        FnState::merge(self, other)
    }
}
