use sa_verify::capability::{CapabilityEngine, EngineError};
use sa_verify::mask::{bit, mask_components, Op, TrapKind};

#[test]
fn a1c_fallible_bit_is_carried_then_resolved() {
    let mut engine = CapabilityEngine::new(2);
    engine.bind_active(0).unwrap();
    engine.bind_fallible(0).unwrap();
    assert_eq!(engine.mask(0), bit::ACTIVE | bit::FALLIBLE);
    assert!(mask_components(engine.mask(0)).contains(&"Fallible"));
    engine.resolve_fallible(0).unwrap();
    assert_eq!(engine.mask(0), bit::ACTIVE);
    engine.bind_fallible(1).unwrap();
    assert_eq!(engine.mask(1), bit::FALLIBLE);
}

#[test]
fn a1c_immutable_bit_survives_alloc_and_blocks_transfer() {
    let mut engine = CapabilityEngine::new(3);
    engine.bind_immutable(0).unwrap();
    assert_eq!(engine.mask(0), bit::IMMUTABLE);
    engine.note_alloc(0).unwrap();
    assert_eq!(engine.mask(0), bit::IMMUTABLE);
    assert_eq!(
        CapabilityEngine::decide(0x100, Op::Alloc),
        Some((true, 0x100, None))
    );

    engine.bind_active(1).unwrap();
    engine.bind_immutable(1).unwrap();
    assert_eq!(engine.mask(1), 0x101);
    assert_eq!(
        mask_components(engine.mask(1)),
        ["Active", "Immutable"]
    );
    engine.note_alloc(2).unwrap();
    assert_eq!(
        engine.transfer_own(2, 1),
        Err(EngineError::TransferIntoImmutable)
    );
    assert_eq!(
        CapabilityEngine::decide(0x101, Op::TransferOwn)
            .unwrap()
            .2,
        Some(TrapKind::TransferIntoImmutable)
    );
}

#[test]
fn a1c_interior_ptr_bit_survives_move() {
    let mut engine = CapabilityEngine::new(2);
    engine.bind_active(0).unwrap();
    engine.note_interior_ptr(0).unwrap();
    assert_eq!(engine.mask(0), 0x201);
    engine.note_move(0).unwrap();
    assert_eq!(engine.mask(0), 0x201);
    assert_eq!(
        CapabilityEngine::decide(0x200, Op::Move),
        Some((true, 0x200, None))
    );
    assert!(mask_components(engine.mask(0)).contains(&"InteriorPtr"));
}

#[test]
fn a1c_transfer_own_sets_owns_children_and_consumes_source() {
    let mut engine = CapabilityEngine::new(2);
    engine.note_alloc(0).unwrap();
    engine.note_alloc(1).unwrap();
    engine.transfer_own(0, 1).unwrap();
    assert_eq!(engine.mask(0), bit::CONSUMED);
    assert_eq!(engine.mask(1), bit::ACTIVE | bit::OWNS_CHILDREN);
    // B5-transfer batch (#19164/#19201): the 0x01 row's new_mask was the
    // old source-side artifact (CONSUMED); the dest-side correction keeps
    // the container ACTIVE and adds owns_children (0x401). The engine's
    // own effect below already wrote exactly that mask.
    assert_eq!(
        CapabilityEngine::decide(0x01, Op::TransferOwn),
        Some((true, 0x401, None))
    );
    assert_eq!(engine.mask(1), 0x401);
}

#[test]
fn a1c_transfer_into_borrow_view_is_rejected() {
    let mut engine = CapabilityEngine::new(4);
    engine.note_alloc(0).unwrap();
    engine.note_alloc(1).unwrap();
    engine.start_read_borrow(1, 2).unwrap();
    assert_eq!(engine.mask(2), 0x11);
    assert_eq!(
        engine.transfer_own(0, 2),
        Err(EngineError::TransferIntoBorrow)
    );
    assert_eq!(
        CapabilityEngine::decide(0x10, Op::TransferOwn)
            .unwrap()
            .2,
        Some(TrapKind::TransferIntoBorrow)
    );
}

#[test]
fn a1c_all_four_derived_bits_are_independent() {
    let mut engine = CapabilityEngine::new(1);
    engine.bind_active(0).unwrap();
    engine.bind_fallible(0).unwrap();
    engine.bind_immutable(0).unwrap();
    engine.note_interior_ptr(0).unwrap();
    engine.bind_fallible(0).unwrap();
    let expected = bit::ACTIVE
        | bit::FALLIBLE
        | bit::IMMUTABLE
        | bit::INTERIOR_PTR;
    assert_eq!(engine.mask(0), expected);
    assert_eq!(
        mask_components(engine.mask(0)),
        ["Active", "Fallible", "Immutable", "InteriorPtr"]
    );
}
