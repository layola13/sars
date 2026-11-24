use crate::program::SourceLoc;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Trap {
    ForbiddenSyntax,
    DuplicateDef,
    DuplicateLabel,
    UnsupportedType,
    ImportResolutionFailed,
    MacroRecursionLimit,
    RegisterRedefinition,
    UnknownRegister,
    BorrowConflict,
    UseAfterMove,
    DoubleMutableBorrow,
    ReadWriteConflict,
    MemoryLeak,
    CapabilityMismatch,
    FallthroughForbidden,
    PhiStateConflict,
    GasExceeded,
    ArenaOom,
    SnapshotVersionMismatch,
    IllegalUnsafeContext,
    FfiOwnershipViolation,
    UnsupportedSysIntrinsic,
    InteriorPtrEscape,
    ConstMutation,
    VTableSignatureMismatch,
    StackEscape,
    UnauthorizedPrimitive,
    UpstreamShaMismatch,
    EarlyReturnLeak,
    FallibleContractMismatch,
    InvalidAtomicOrdering,
    AtomicOrderingMismatch,
    TestFuncSignatureMismatch,
    DbCapabilityEscalation,
    DbMemoryGuardViolation,
    DbBlobArenaOom,
    DbConcurrencyConflict,
    DbSchemaMismatch,
    DbCursorOverflow,
    DbColumnTypeMismatch,
    DbQueryHashUnknown,
    DbBlobHandleInvalid,
    DbSnapshotCorrupted,
    DbDuplicateRegister,
    DbForbiddenSqlString,
    SaxStateLeak,
    SaxEventEscape,
    SaxRenderOutsideHandler,
    SaxInvalidInterpolation,
    SaxStateWriteFromOutside,
    SaxUnknownTag,
    SaxUnknownEvent,
    MachineCodeHashMismatch,
    BlockedRiskUnconfirmed,
    MissingTtyForConfirmation,
    ForbiddenGlobalConfig,
    SumHashMismatch,
    CorruptedSignature,
    GpuSubsetViolation,
    UndefinedMacro,
    TransferIntoBorrow,
    TransferIntoImmutable,
    ParseError,
    UnsupportedInstruction,
    DivisionByZero,
    RuntimeError,
}

impl Trap {
    pub fn name(self) -> &'static str {
        match self {
            Self::ForbiddenSyntax => "ForbiddenSyntax",
            Self::DuplicateDef => "DuplicateDef",
            Self::DuplicateLabel => "DuplicateLabel",
            Self::UnsupportedType => "UnsupportedType",
            Self::ImportResolutionFailed => "ImportResolutionFailed",
            Self::MacroRecursionLimit => "MacroRecursionLimit",
            Self::RegisterRedefinition => "RegisterRedefinition",
            Self::UnknownRegister => "UnknownRegister",
            Self::BorrowConflict => "BorrowConflict",
            Self::UseAfterMove => "UseAfterMove",
            Self::DoubleMutableBorrow => "DoubleMutableBorrow",
            Self::ReadWriteConflict => "ReadWriteConflict",
            Self::MemoryLeak => "MemoryLeak",
            Self::CapabilityMismatch => "CapabilityMismatch",
            Self::FallthroughForbidden => "FallthroughForbidden",
            Self::PhiStateConflict => "PhiStateConflict",
            Self::GasExceeded => "GasExceeded",
            Self::ArenaOom => "ArenaOOM",
            Self::SnapshotVersionMismatch => "SnapshotVersionMismatch",
            Self::IllegalUnsafeContext => "IllegalUnsafeContext",
            Self::FfiOwnershipViolation => "FfiOwnershipViolation",
            Self::UnsupportedSysIntrinsic => "UnsupportedSysIntrinsic",
            Self::InteriorPtrEscape => "InteriorPtrEscape",
            Self::ConstMutation => "ConstMutation",
            Self::VTableSignatureMismatch => "VTableSignatureMismatch",
            Self::StackEscape => "StackEscape",
            Self::UnauthorizedPrimitive => "UnauthorizedPrimitive",
            Self::UpstreamShaMismatch => "UpstreamShaMismatch",
            Self::EarlyReturnLeak => "EarlyReturnLeak",
            Self::FallibleContractMismatch => "FallibleContractMismatch",
            Self::InvalidAtomicOrdering => "InvalidAtomicOrdering",
            Self::AtomicOrderingMismatch => "AtomicOrderingMismatch",
            Self::TestFuncSignatureMismatch => "TestFuncSignatureMismatch",
            Self::DbCapabilityEscalation => "DbCapabilityEscalation",
            Self::DbMemoryGuardViolation => "DbMemoryGuardViolation",
            Self::DbBlobArenaOom => "DbBlobArenaOOM",
            Self::DbConcurrencyConflict => "DbConcurrencyConflict",
            Self::DbSchemaMismatch => "DbSchemaMismatch",
            Self::DbCursorOverflow => "DbCursorOverflow",
            Self::DbColumnTypeMismatch => "DbColumnTypeMismatch",
            Self::DbQueryHashUnknown => "DbQueryHashUnknown",
            Self::DbBlobHandleInvalid => "DbBlobHandleInvalid",
            Self::DbSnapshotCorrupted => "DbSnapshotCorrupted",
            Self::DbDuplicateRegister => "DbDuplicateRegister",
            Self::DbForbiddenSqlString => "DbForbiddenSqlString",
            Self::SaxStateLeak => "SaxStateLeak",
            Self::SaxEventEscape => "SaxEventEscape",
            Self::SaxRenderOutsideHandler => "SaxRenderOutsideHandler",
            Self::SaxInvalidInterpolation => "SaxInvalidInterpolation",
            Self::SaxStateWriteFromOutside => "SaxStateWriteFromOutside",
            Self::SaxUnknownTag => "SaxUnknownTag",
            Self::SaxUnknownEvent => "SaxUnknownEvent",
            Self::MachineCodeHashMismatch => "MachineCodeHashMismatch",
            Self::BlockedRiskUnconfirmed => "BlockedRiskUnconfirmed",
            Self::MissingTtyForConfirmation => "MissingTtyForConfirmation",
            Self::ForbiddenGlobalConfig => "ForbiddenGlobalConfig",
            Self::SumHashMismatch => "SumHashMismatch",
            Self::CorruptedSignature => "CorruptedSignature",
            Self::GpuSubsetViolation => "GpuSubsetViolation",
            Self::UndefinedMacro => "UndefinedMacro",
            Self::TransferIntoBorrow => "TransferIntoBorrow",
            Self::TransferIntoImmutable => "TransferIntoImmutable",
            Self::ParseError => "ParseError",
            Self::UnsupportedInstruction => "UnsupportedInstruction",
            Self::DivisionByZero => "DivisionByZero",
            Self::RuntimeError => "RuntimeError",
        }
    }

    pub fn code(self) -> u32 {
        match self {
            Self::ForbiddenSyntax => 1001,
            Self::DuplicateDef => 1002,
            Self::DuplicateLabel => 1003,
            Self::UnsupportedType => 1004,
            Self::MacroRecursionLimit => 1005,
            Self::RegisterRedefinition => 1006,
            Self::UnknownRegister => 1007,
            Self::BorrowConflict => 1008,
            Self::UseAfterMove => 1009,
            Self::DoubleMutableBorrow => 1010,
            Self::ReadWriteConflict => 1011,
            Self::MemoryLeak => 1012,
            Self::CapabilityMismatch => 1013,
            Self::FallthroughForbidden => 1014,
            Self::PhiStateConflict => 1015,
            Self::GasExceeded => 1016,
            Self::ArenaOom => 1017,
            Self::SnapshotVersionMismatch => 1018,
            Self::IllegalUnsafeContext => 1019,
            Self::FfiOwnershipViolation => 1020,
            Self::UnsupportedSysIntrinsic => 1021,
            Self::InteriorPtrEscape => 1022,
            Self::ConstMutation => 1023,
            Self::VTableSignatureMismatch => 1024,
            Self::StackEscape => 1025,
            Self::EarlyReturnLeak => 1026,
            Self::FallibleContractMismatch => 1027,
            Self::InvalidAtomicOrdering => 1028,
            Self::AtomicOrderingMismatch => 1029,
            Self::TestFuncSignatureMismatch => 1030,
            Self::DbCapabilityEscalation => 1031,
            Self::DbMemoryGuardViolation => 1032,
            Self::DbBlobArenaOom => 1033,
            Self::DbConcurrencyConflict => 1034,
            Self::DbSchemaMismatch => 1035,
            Self::DbCursorOverflow => 1036,
            Self::DbColumnTypeMismatch => 1037,
            Self::DbQueryHashUnknown => 1038,
            Self::DbBlobHandleInvalid => 1039,
            Self::DbSnapshotCorrupted => 1040,
            Self::DbDuplicateRegister => 1041,
            Self::DbForbiddenSqlString => 1042,
            Self::UnauthorizedPrimitive => 1043,
            Self::UpstreamShaMismatch => 1044,
            Self::ImportResolutionFailed => 1050,
            Self::SaxUnknownTag => 1051,
            Self::SaxUnknownEvent => 1052,
            Self::MachineCodeHashMismatch => 1053,
            Self::BlockedRiskUnconfirmed => 1054,
            Self::MissingTtyForConfirmation => 1055,
            Self::ForbiddenGlobalConfig => 1056,
            Self::SumHashMismatch => 1057,
            Self::CorruptedSignature => 1058,
            Self::GpuSubsetViolation => 1059,
            Self::UndefinedMacro => 1060,
            Self::TransferIntoBorrow => 1061,
            Self::TransferIntoImmutable => 1062,
            Self::SaxStateLeak => 1045,
            Self::SaxEventEscape => 1046,
            Self::SaxRenderOutsideHandler => 1047,
            Self::SaxInvalidInterpolation => 1048,
            Self::SaxStateWriteFromOutside => 1049,
            Self::ParseError => 2001,
            Self::UnsupportedInstruction => 2002,
            Self::DivisionByZero => 2003,
            Self::RuntimeError => 2999,
        }
    }
}

impl fmt::Display for Trap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub trap: Trap,
    pub message: String,
    pub loc: SourceLoc,
    pub hint: Option<String>,
    pub function: Option<String>,
    pub register: Option<String>,
}

impl Diagnostic {
    pub fn new(trap: Trap, message: impl Into<String>, loc: SourceLoc) -> Self {
        Self {
            trap,
            message: message.into(),
            loc,
            hint: None,
            function: None,
            register: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn in_function(mut self, name: impl Into<String>) -> Self {
        self.function = Some(name.into());
        self
    }

    pub fn for_register(mut self, name: impl Into<String>) -> Self {
        self.register = Some(name.into());
        self
    }

    pub fn code(&self) -> u32 {
        self.trap.code()
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[E{}]: {}", self.loc, self.code(), self.message)?;
        if let Some(func) = &self.function {
            write!(f, " (in {func})")?;
        }
        if let Some(hint) = &self.hint {
            write!(f, "\n  hint: {hint}")?;
        }
        Ok(())
    }
}
