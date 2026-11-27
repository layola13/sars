use crate::mask::{bit, lookup, Op, TrapKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineError {
    UnknownRegister,
    BorrowConflict,
    UseAfterMove,
    DoubleMutableBorrow,
    ReadWriteConflict,
    RegisterRedefinition,
    MemoryLeak,
    FfiOwnershipViolation,
    TransferIntoBorrow,
    TransferIntoImmutable,
}

impl EngineError {
    pub fn from_trap(trap: TrapKind) -> Self {
        match trap {
            TrapKind::ReadWriteConflict => EngineError::ReadWriteConflict,
            TrapKind::BorrowConflict => EngineError::BorrowConflict,
            TrapKind::DoubleMutableBorrow => EngineError::DoubleMutableBorrow,
            TrapKind::UseAfterMove => EngineError::UseAfterMove,
            TrapKind::MemoryLeak => EngineError::MemoryLeak,
            TrapKind::FfiOwnershipViolation => EngineError::FfiOwnershipViolation,
            TrapKind::TransferIntoBorrow => EngineError::TransferIntoBorrow,
            TrapKind::TransferIntoImmutable => EngineError::TransferIntoImmutable,
            TrapKind::IllegalUnsafeContext | TrapKind::StackEscape => EngineError::BorrowConflict,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CapabilityEngine {
    masks: Vec<u16>,
    origins: Vec<Option<usize>>,
    lock_refs: Vec<u16>,
    flags: Vec<u8>,
}

impl CapabilityEngine {
    pub fn new(count: usize) -> Self {
        Self {
            masks: vec![bit::UNINITIALIZED; count],
            origins: vec![None; count],
            lock_refs: vec![0; count],
            flags: vec![0; count],
        }
    }

    pub fn mask(&self, id: usize) -> u16 {
        self.masks[id]
    }

    pub fn masks(&self) -> &[u16] {
        &self.masks
    }

    pub fn origin(&self, id: usize) -> Option<usize> {
        self.origins[id]
    }

    pub fn lock_refs(&self, id: usize) -> u16 {
        self.lock_refs[id]
    }

    fn ensure(&self, id: usize) -> Result<usize, EngineError> {
        if id >= self.masks.len() {
            Err(EngineError::UnknownRegister)
        } else {
            Ok(id)
        }
    }

    pub fn decide(prev_mask: u16, op: Op) -> Option<(bool, u16, Option<TrapKind>)> {
        lookup(prev_mask, op).map(|row| (row.legal, row.new_mask, row.trap))
    }

    pub fn bind_active(&mut self, id: usize) -> Result<(), EngineError> {
        let index = self.ensure(id)?;
        self.masks[index] = bit::ACTIVE;
        self.origins[index] = None;
        self.lock_refs[index] = 0;
        self.flags[index] = 0;
        Ok(())
    }

    pub fn bind_raw(&mut self, id: usize) -> Result<(), EngineError> {
        let index = self.ensure(id)?;
        self.masks[index] = bit::UNTRACKED;
        self.origins[index] = None;
        self.lock_refs[index] = 0;
        self.flags[index] = 0;
        Ok(())
    }

    pub fn note_alloc(&mut self, id: usize) -> Result<(), EngineError> {
        let index = self.ensure(id)?;
        let current = self.masks[index];
        if current & bit::IMMUTABLE != 0 {
            return Ok(());
        }
        if current != 0 && current != bit::CONSUMED && current != bit::UNTRACKED {
            return Err(EngineError::RegisterRedefinition);
        }
        self.masks[index] = bit::ACTIVE;
        self.origins[index] = None;
        self.lock_refs[index] = 0;
        self.flags[index] = 0;
        Ok(())
    }

    pub fn note_read(&mut self, id: usize) -> Result<(), EngineError> {
        let index = self.ensure(id)?;
        let current = self.masks[index];
        if current & bit::UNTRACKED != 0 {
            return Ok(());
        }
        match current {
            0x00 => Err(EngineError::UnknownRegister),
            0x08 => Err(EngineError::UseAfterMove),
            0x04 => Err(EngineError::BorrowConflict),
            _ => Ok(()),
        }
    }

    pub fn note_write(&mut self, id: usize) -> Result<(), EngineError> {
        let index = self.ensure(id)?;
        let current = self.masks[index];
        if current & bit::UNTRACKED != 0 {
            return Ok(());
        }
        match current {
            0x00 => Err(EngineError::UnknownRegister),
            0x08 => Err(EngineError::UseAfterMove),
            0x02 => Err(EngineError::ReadWriteConflict),
            0x04 => Err(EngineError::BorrowConflict),
            _ => Ok(()),
        }
    }

    pub fn note_borrow_view(&mut self, id: usize) -> Result<(), EngineError> {
        let index = self.ensure(id)?;
        match self.masks[index] {
            0x00 => Err(EngineError::UnknownRegister),
            0x08 => Err(EngineError::UseAfterMove),
            _ => {
                self.masks[index] |= bit::BORROW_VIEW;
                Ok(())
            }
        }
    }

    pub fn note_move_view(&mut self, id: usize) -> Result<(), EngineError> {
        let index = self.ensure(id)?;
        let current = self.masks[index];
        if current & bit::FFI_BORROW != 0 {
            return Err(EngineError::FfiOwnershipViolation);
        }
        match current {
            0x00 => Err(EngineError::UnknownRegister),
            0x08 => Err(EngineError::UseAfterMove),
            0x02 | 0x04 => Err(EngineError::BorrowConflict),
            0x40 => Ok(()),
            _ => {
                self.masks[index] = bit::CONSUMED;
                Ok(())
            }
        }
    }

    pub fn note_move(&mut self, id: usize) -> Result<(), EngineError> {
        let index = self.ensure(id)?;
        let current = self.masks[index];
        if current & bit::FFI_BORROW != 0 {
            return Err(EngineError::FfiOwnershipViolation);
        }
        if current & bit::INTERIOR_PTR != 0 {
            return Ok(());
        }
        match current {
            0x00 => Err(EngineError::UnknownRegister),
            0x08 => Err(EngineError::UseAfterMove),
            0x02 | 0x04 => Err(EngineError::BorrowConflict),
            0x40 => Ok(()),
            _ => {
                self.masks[index] = bit::CONSUMED;
                Ok(())
            }
        }
    }

    pub fn note_release_own(&mut self, id: usize) -> Result<(), EngineError> {
        let index = self.ensure(id)?;
        if self.masks[index] & bit::FFI_BORROW != 0 {
            return Err(EngineError::FfiOwnershipViolation);
        }
        match self.masks[index] {
            0x00 => Err(EngineError::UnknownRegister),
            0x08 => Err(EngineError::UseAfterMove),
            0x02 | 0x04 => Err(EngineError::BorrowConflict),
            0x40 => Ok(()),
            _ => {
                self.masks[index] = bit::CONSUMED;
                Ok(())
            }
        }
    }

    pub fn start_read_borrow(&mut self, source_id: usize, view_id: usize) -> Result<(), EngineError> {
        let source = self.ensure(source_id)?;
        let view = self.ensure(view_id)?;
        match self.masks[source] {
            0x00 => Err(EngineError::UnknownRegister),
            0x08 => Err(EngineError::UseAfterMove),
            0x04 => Err(EngineError::BorrowConflict),
            0x40 => {
                self.masks[view] = bit::ACTIVE | bit::BORROW_VIEW | bit::LOCKED_READ | bit::FFI_BORROW;
                self.origins[view] = Some(source_id);
                self.lock_refs[source] += 1;
                Ok(())
            }
            _ => {
                if self.masks[source] == bit::ACTIVE {
                    self.masks[source] = bit::LOCKED_READ;
                }
                self.masks[view] = bit::ACTIVE | bit::BORROW_VIEW;
                self.origins[view] = Some(source_id);
                self.lock_refs[source] += 1;
                Ok(())
            }
        }
    }

    pub fn start_mut_borrow(&mut self, source_id: usize, view_id: usize) -> Result<(), EngineError> {
        let source = self.ensure(source_id)?;
        let view = self.ensure(view_id)?;
        match self.masks[source] {
            0x00 => Err(EngineError::UnknownRegister),
            0x08 => Err(EngineError::UseAfterMove),
            0x04 => Err(EngineError::DoubleMutableBorrow),
            0x02 => Err(EngineError::ReadWriteConflict),
            0x40 => {
                self.masks[view] = bit::ACTIVE | bit::BORROW_VIEW | bit::LOCKED_MUT | bit::FFI_BORROW;
                self.origins[view] = Some(source_id);
                self.lock_refs[source] += 1;
                Ok(())
            }
            _ => {
                self.masks[source] = bit::LOCKED_MUT;
                self.masks[view] = bit::ACTIVE | bit::BORROW_VIEW;
                self.origins[view] = Some(source_id);
                self.lock_refs[source] += 1;
                Ok(())
            }
        }
    }

    pub fn release_borrow(&mut self, view_id: usize) -> Result<(), EngineError> {
        let view = self.ensure(view_id)?;
        let origin_id = self.origins[view].ok_or(EngineError::UnknownRegister)?;
        let origin = self.ensure(origin_id)?;
        if self.masks[view] & bit::BORROW_VIEW == 0 {
            return Err(EngineError::UnknownRegister);
        }
        if self.lock_refs[origin] == 0 {
            return Err(EngineError::UnknownRegister);
        }
        self.lock_refs[origin] -= 1;
        if self.lock_refs[origin] == 0 {
            let origin_mask = self.masks[origin];
            if origin_mask & bit::BORROW_VIEW == 0
                && origin_mask & (bit::LOCKED_READ | bit::LOCKED_MUT) != 0
            {
                self.masks[origin] = bit::ACTIVE;
            }
        }
        self.masks[view] = bit::UNINITIALIZED;
        self.origins[view] = None;
        self.flags[view] = 0;
        Ok(())
    }

    pub fn bind_fallible(&mut self, id: usize) -> Result<(), EngineError> {
        let index = self.ensure(id)?;
        self.masks[index] |= bit::FALLIBLE;
        Ok(())
    }

    pub fn resolve_fallible(&mut self, id: usize) -> Result<(), EngineError> {
        let index = self.ensure(id)?;
        self.masks[index] &= !bit::FALLIBLE;
        Ok(())
    }

    pub fn bind_immutable(&mut self, id: usize) -> Result<(), EngineError> {
        let index = self.ensure(id)?;
        self.masks[index] |= bit::IMMUTABLE;
        Ok(())
    }

    pub fn note_interior_ptr(&mut self, id: usize) -> Result<(), EngineError> {
        let index = self.ensure(id)?;
        self.masks[index] |= bit::INTERIOR_PTR;
        Ok(())
    }

    pub fn transfer_own(&mut self, src: usize, dest: usize) -> Result<(), EngineError> {
        let src = self.ensure(src)?;
        let dest = self.ensure(dest)?;
        let src_mask = self.masks[src];
        if src_mask == bit::UNINITIALIZED {
            return Err(EngineError::UnknownRegister);
        }
        if src_mask == bit::CONSUMED {
            return Err(EngineError::UseAfterMove);
        }
        if src_mask & (bit::LOCKED_READ | bit::LOCKED_MUT | bit::BORROW_VIEW) != 0 {
            return Err(EngineError::BorrowConflict);
        }
        if src_mask & bit::FFI_BORROW != 0 {
            return Err(EngineError::FfiOwnershipViolation);
        }
        let dest_mask = self.masks[dest];
        if dest_mask & bit::BORROW_VIEW != 0 {
            return Err(EngineError::TransferIntoBorrow);
        }
        if dest_mask & bit::IMMUTABLE != 0 {
            return Err(EngineError::TransferIntoImmutable);
        }
        let (legal, _, trap) = Self::decide(dest_mask, Op::TransferOwn)
            .ok_or(EngineError::RegisterRedefinition)?;
        if !legal {
            return Err(trap.map(EngineError::from_trap).unwrap_or(EngineError::BorrowConflict));
        }
        self.masks[src] = bit::CONSUMED;
        self.masks[dest] = dest_mask | bit::OWNS_CHILDREN;
        self.origins[dest] = None;
        Ok(())
    }

    pub fn finalize_leaks(&self) -> Result<(), EngineError> {
        for &mask in &self.masks {
            if mask == 0 || mask == bit::CONSUMED || mask == bit::UNTRACKED {
                continue;
            }
            return Err(EngineError::MemoryLeak);
        }
        Ok(())
    }

    pub fn first_live_register(&self) -> Option<usize> {
        self.masks.iter().position(|&mask| mask != 0 && mask != bit::CONSUMED)
    }

    pub fn snapshot(&self) -> crate::merge::MaskSnapshot {
        crate::merge::MaskSnapshot {
            masks: self.masks.clone(),
            origins: self.origins.clone(),
            lock_refs: self.lock_refs.clone(),
            flags: self.flags.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &crate::merge::MaskSnapshot) {
        self.masks = snapshot.masks.clone();
        self.origins = snapshot.origins.clone();
        self.lock_refs = snapshot.lock_refs.clone();
        self.flags = snapshot.flags.clone();
    }
}
