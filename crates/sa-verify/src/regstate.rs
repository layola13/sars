use crate::mask::{bit, CapabilityMask};
use crate::state::{merge_reg, RegKind, RegState, RegStateBackend};
use std::collections::HashMap;

const FLAG_FALLIBLE_PRESENT: u8 = 0b0001;
const FLAG_FALLIBLE_TRUE: u8 = 0b0010;
const FLAG_ESCAPED: u8 = 0b0100;
const FLAG_BORROW_PARAM: u8 = 0b1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variant {
    Undefined,
    Owned,
    Moved,
    Borrowed,
    Released,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapEntry {
    pub mask: Option<CapabilityMask>,
    variant: Variant,
    kind: RegKind,
    lock_refs: u16,
    mutable: bool,
    origin: Option<String>,
    base: Option<RegState>,
    flags: u8,
}

impl CapEntry {
    fn from_state(state: RegState) -> Self {
        let mut entry = Self {
            mask: None,
            variant: Variant::Undefined,
            kind: RegKind::Value,
            lock_refs: 0,
            mutable: false,
            origin: None,
            base: None,
            flags: 0,
        };
        entry.apply_state(state);
        entry
    }

    fn apply_state(&mut self, state: RegState) {
        match state {
            RegState::Undefined => {
                self.variant = Variant::Undefined;
                self.kind = RegKind::Value;
                self.lock_refs = 0;
            }
            RegState::Owned { kind } => {
                self.variant = Variant::Owned;
                self.kind = kind;
                self.lock_refs = 0;
            }
            RegState::Moved => {
                self.variant = Variant::Moved;
                self.kind = RegKind::Value;
                self.lock_refs = 0;
            }
            RegState::Borrowed { shared, mutable } => {
                self.variant = Variant::Borrowed;
                self.kind = RegKind::Value;
                self.lock_refs = shared as u16;
                self.mutable = mutable;
            }
            RegState::Released => {
                self.variant = Variant::Released;
                self.kind = RegKind::Value;
                self.lock_refs = 0;
            }
            RegState::Conflict => {
                self.variant = Variant::Conflict;
                self.kind = RegKind::Value;
                self.lock_refs = 0;
            }
        }
    }

    fn state(&self) -> RegState {
        match self.variant {
            Variant::Undefined => RegState::Undefined,
            Variant::Owned => RegState::Owned { kind: self.kind },
            Variant::Moved => RegState::Moved,
            Variant::Borrowed => RegState::Borrowed {
                shared: self.lock_refs as usize,
                mutable: self.mutable,
            },
            Variant::Released => RegState::Released,
            Variant::Conflict => RegState::Conflict,
        }
    }

    fn project_mask(&self) -> u16 {
        match self.state() {
            RegState::Undefined | RegState::Conflict => bit::UNINITIALIZED,
            RegState::Owned { kind: RegKind::Scalar } => bit::UNTRACKED,
            RegState::Owned { .. } => bit::ACTIVE,
            RegState::Moved | RegState::Released => bit::CONSUMED,
            RegState::Borrowed { shared, mutable } => {
                if self.origin.is_some() {
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

    fn effective_mask(&self) -> u16 {
        self.mask
            .map(CapabilityMask::bits)
            .unwrap_or_else(|| self.project_mask())
    }

    fn fallible(&self) -> Option<bool> {
        if self.flags & FLAG_FALLIBLE_PRESENT == 0 {
            None
        } else {
            Some(self.flags & FLAG_FALLIBLE_TRUE != 0)
        }
    }

    fn set_fallible(&mut self, value: Option<bool>) {
        self.flags &= !(FLAG_FALLIBLE_PRESENT | FLAG_FALLIBLE_TRUE);
        if let Some(value) = value {
            self.flags |= FLAG_FALLIBLE_PRESENT;
            if value {
                self.flags |= FLAG_FALLIBLE_TRUE;
            }
        }
    }

    fn escaped(&self) -> bool {
        self.flags & FLAG_ESCAPED != 0
    }

    fn borrow_param(&self) -> bool {
        self.flags & FLAG_BORROW_PARAM != 0
    }
}

#[derive(Debug, Clone, Default)]
pub struct CapabilityState {
    entries: HashMap<String, CapEntry>,
}

impl CapabilityState {
    pub fn entry(&self, reg: &str) -> Option<&CapEntry> {
        self.entries.get(reg)
    }

    pub fn entries(&self) -> &HashMap<String, CapEntry> {
        &self.entries
    }

    pub fn capability_mask(&self, reg: &str) -> CapabilityMask {
        CapabilityMask::new(self.mask_of(reg))
    }

    fn entry_mut(&mut self, reg: &str) -> &mut CapEntry {
        self.entries
            .entry(reg.to_string())
            .or_insert_with(|| CapEntry::from_state(RegState::Undefined))
    }

    fn merge_entry(&self, other: &CapabilityState, key: &str) -> Option<CapEntry> {
        let left = self.entries.get(key);
        let right = other.entries.get(key);
        let left_state = left.map(CapEntry::state).unwrap_or(RegState::Undefined);
        let right_state = right.map(CapEntry::state).unwrap_or(RegState::Undefined);
        let merged = merge_reg(&left_state, &right_state);
        let mut entry = if merged != RegState::Undefined {
            Some(CapEntry::from_state(merged))
        } else {
            None
        };
        fn ensure(entry: &mut Option<CapEntry>) -> &mut CapEntry {
            entry.get_or_insert_with(|| CapEntry::from_state(RegState::Undefined))
        }
        let has_mask = left.map(|e| e.mask.is_some()).unwrap_or(false)
            || right.map(|e| e.mask.is_some()).unwrap_or(false);
        if has_mask {
            let left_mask = left.map(CapEntry::effective_mask).unwrap_or(bit::UNINITIALIZED);
            let right_mask = right.map(CapEntry::effective_mask).unwrap_or(bit::UNINITIALIZED);
            let joined = crate::merge::join_masks(left_mask, right_mask).0;
            if joined != bit::UNINITIALIZED {
                ensure(&mut entry).mask = Some(CapabilityMask::new(joined));
            }
        }
        let fallible = match (left.and_then(CapEntry::fallible), right.and_then(CapEntry::fallible)) {
            (Some(a), Some(b)) if a == b => Some(a),
            _ => None,
        };
        let origin = match (left.and_then(|e| e.origin.as_ref()), right.and_then(|e| e.origin.as_ref())) {
            (Some(a), Some(b)) if a == b => Some(a.clone()),
            _ => None,
        };
        let base = match (left.and_then(|e| e.base.as_ref()), right.and_then(|e| e.base.as_ref())) {
            (Some(a), Some(b)) if a == b => Some(a.clone()),
            _ => None,
        };
        let escaped = left.map(CapEntry::escaped).unwrap_or(false)
            || right.map(CapEntry::escaped).unwrap_or(false);
        let borrow_param = left.map(CapEntry::borrow_param).unwrap_or(false)
            || right.map(CapEntry::borrow_param).unwrap_or(false);
        if fallible.is_some() || origin.is_some() || base.is_some() || escaped || borrow_param {
            let merged_entry = ensure(&mut entry);
            merged_entry.set_fallible(fallible);
            merged_entry.origin = origin;
            merged_entry.base = base;
            if escaped {
                merged_entry.flags |= FLAG_ESCAPED;
            }
            if borrow_param {
                merged_entry.flags |= FLAG_BORROW_PARAM;
            }
        }
        entry
    }
}

impl RegStateBackend for CapabilityState {
    fn state_of(&self, reg: &str) -> RegState {
        self.entries
            .get(reg)
            .map(CapEntry::state)
            .unwrap_or(RegState::Undefined)
    }

    fn set(&mut self, reg: &str, state: RegState) {
        self.entry_mut(reg).apply_state(state);
    }

    fn mask_of(&self, reg: &str) -> u16 {
        self.entries
            .get(reg)
            .map(CapEntry::effective_mask)
            .unwrap_or(bit::UNINITIALIZED)
    }

    fn set_mask(&mut self, reg: &str, mask: u16) {
        self.entry_mut(reg).mask = Some(CapabilityMask::new(mask));
    }

    fn fallible_get(&self, reg: &str) -> Option<bool> {
        self.entries.get(reg).and_then(CapEntry::fallible)
    }

    fn fallible_insert(&mut self, reg: &str, value: bool) {
        self.entry_mut(reg).set_fallible(Some(value));
    }

    fn borrow_origin_contains(&self, reg: &str) -> bool {
        self.entries
            .get(reg)
            .map(|entry| entry.origin.is_some())
            .unwrap_or(false)
    }

    fn borrow_origin_insert(&mut self, dst: &str, origin: &str) {
        self.entry_mut(dst).origin = Some(origin.to_string());
    }

    fn borrow_origin_remove(&mut self, reg: &str) -> Option<String> {
        self.entries.get_mut(reg).and_then(|entry| entry.origin.take())
    }

    fn origin_base_entry_or_insert(&mut self, reg: &str, state: RegState) {
        let entry = self.entry_mut(reg);
        if entry.base.is_none() {
            entry.base = Some(state);
        }
    }

    fn origin_base_get(&self, reg: &str) -> Option<RegState> {
        self.entries.get(reg).and_then(|entry| entry.base.clone())
    }

    fn origin_base_remove(&mut self, reg: &str) -> Option<RegState> {
        self.entries.get_mut(reg).and_then(|entry| entry.base.take())
    }

    fn escaped_contains(&self, reg: &str) -> bool {
        self.entries.get(reg).map(CapEntry::escaped).unwrap_or(false)
    }

    fn escaped_insert(&mut self, reg: &str) {
        self.entry_mut(reg).flags |= FLAG_ESCAPED;
    }

    fn borrow_params_contains(&self, reg: &str) -> bool {
        self.entries.get(reg).map(CapEntry::borrow_param).unwrap_or(false)
    }

    fn borrow_params_insert(&mut self, reg: &str) {
        self.entry_mut(reg).flags |= FLAG_BORROW_PARAM;
    }

    fn regs_snapshot(&self) -> Vec<(String, RegState)> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.variant != Variant::Undefined)
            .map(|(name, entry)| (name.clone(), entry.state()))
            .collect()
    }

    fn is_same(&self, other: &Self) -> bool {
        self.entries == other.entries
    }

    fn merge(&self, other: &Self) -> (Self, Vec<(String, RegState, RegState)>) {
        let mut keys: Vec<&String> = self.entries.keys().chain(other.entries.keys()).collect();
        keys.sort();
        keys.dedup();
        let mut out = CapabilityState::default();
        let mut conflicts = Vec::new();
        for key in keys {
            let left = self.entries.get(key);
            let right = other.entries.get(key);
            let a = left.map(CapEntry::state).unwrap_or(RegState::Undefined);
            let b = right.map(CapEntry::state).unwrap_or(RegState::Undefined);
            let merged = merge_reg(&a, &b);
            if merged == RegState::Conflict && a != b {
                if a != RegState::Undefined && b != RegState::Undefined {
                    conflicts.push((key.clone(), a, b));
                }
            }
            if let Some(entry) = self.merge_entry(other, key) {
                out.entries.insert(key.clone(), entry);
            }
        }
        (out, conflicts)
    }
}

impl From<&crate::state::FnState> for CapabilityState {
    fn from(legacy: &crate::state::FnState) -> Self {
        let mut keys: Vec<&String> = legacy.regs.keys().chain(legacy.masks.keys()).collect();
        keys.extend(legacy.fallible.keys());
        keys.extend(legacy.borrow_origin.keys());
        keys.extend(legacy.origin_base.keys());
        keys.extend(legacy.escaped.iter());
        keys.extend(legacy.borrow_params.iter());
        keys.sort();
        keys.dedup();
        let mut out = CapabilityState::default();
        for key in keys {
            let mut entry = CapEntry::from_state(legacy.state_of(key));
            if let Some(mask) = legacy.masks.get(key) {
                entry.mask = Some(CapabilityMask::new(*mask));
            }
            entry.set_fallible(legacy.fallible.get(key).copied());
            entry.origin = legacy.borrow_origin.get(key).cloned();
            entry.base = legacy.origin_base.get(key).cloned();
            if legacy.escaped.contains(key) {
                entry.flags |= FLAG_ESCAPED;
            }
            if legacy.borrow_params.contains(key) {
                entry.flags |= FLAG_BORROW_PARAM;
            }
            out.entries.insert(key.clone(), entry);
        }
        out
    }
}

impl From<&CapabilityState> for crate::state::FnState {
    fn from(collapsed: &CapabilityState) -> Self {
        let mut out = crate::state::FnState::default();
        for (key, entry) in &collapsed.entries {
            if entry.variant != Variant::Undefined {
                out.regs.insert(key.clone(), entry.state());
            }
            if let Some(mask) = entry.mask {
                out.masks.insert(key.clone(), mask.bits());
            }
            if let Some(value) = entry.fallible() {
                out.fallible.insert(key.clone(), value);
            }
            if let Some(origin) = &entry.origin {
                out.borrow_origin.insert(key.clone(), origin.clone());
            }
            if let Some(base) = &entry.base {
                out.origin_base.insert(key.clone(), base.clone());
            }
            if entry.escaped() {
                out.escaped.insert(key.clone());
            }
            if entry.borrow_param() {
                out.borrow_params.insert(key.clone());
            }
        }
        out
    }
}
