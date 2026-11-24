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

    pub fn is_same(&self, other: &FnState) -> bool {
        self.regs == other.regs
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

fn merge_reg(a: &RegState, b: &RegState) -> RegState {
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
