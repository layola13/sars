use sa_verify::mask::{Op, TRUTH_TABLE};
use sa_verify::report::report_from_decision;

#[test]
fn a1e_report_carries_register_expected_and_actual_mask_names() {
    let report = report_from_decision("a", Op::BorrowMut, 0x02, 12)
        .expect("shared-then-mutable borrow must report");
    assert_eq!(report.trap_name(), "ReadWriteConflict");
    assert_eq!(report.register, "a");
    assert_eq!(report.line, 12);
    assert_eq!(report.expected_mask, 0x01);
    assert_eq!(report.expected_mask_name(), "Active");
    assert_eq!(report.actual_mask, 0x02);
    assert_eq!(report.actual_mask_name(), "Locked_Read");
    assert!(report.message.contains("Active"));
    assert!(report.message.contains("Locked_Read"));
    assert!(report.hint.contains("release the shared borrow"));
}

#[test]
fn a1e_json_follows_sci_trap_report_field_shape() {
    let report = report_from_decision("r1", Op::Alloc, 0x08, 12)
        .expect("use-after-move must report");
    let json = report.to_json();
    let fields = [
        "\"trap\":\"UseAfterMove\"",
        "\"trap_code\":null",
        "\"line\":12",
        "\"source_line\":12",
        "\"source_text\":null",
        "\"original_text\":null",
        "\"register\":\"r1\"",
        "\"registers\":[]",
        "\"expected_mask\":0",
        "\"actual_mask\":8",
        "\"expected_mask_name\":\"Uninitialized\"",
        "\"actual_mask_name\":\"Consumed\"",
        "\"upstream_loc\":null",
        "\"function\":null",
        "\"is_ffi_wrapper\":null",
        "\"message\":",
        "\"hint\":",
    ];
    let mut cursor = 0usize;
    for field in fields {
        let found = json[cursor..].find(field).expect("field in order");
        cursor += found + field.len();
    }
    assert!(json.ends_with('}'));
}

#[test]
fn a1e_composite_mask_reports_composite_name_not_a_single_bit() {
    let report = report_from_decision("box", Op::TransferOwn, 0x101, 7)
        .expect("transfer into immutable must report");
    assert_eq!(report.trap_name(), "TransferIntoImmutable");
    assert_eq!(report.actual_mask, 0x101);
    assert_eq!(report.actual_mask_name(), "Composite");
    assert_ne!(report.actual_mask_name(), "Immutable");
}

#[test]
fn a1e_legal_and_unlisted_pairs_produce_no_report() {
    assert!(report_from_decision("a", Op::Alloc, 0x00, 2).is_none());
    assert!(report_from_decision("a", Op::BorrowRead, 0x01, 2).is_none());
    assert!(report_from_decision("a", Op::Alloc, 0x02, 2).is_none());
}

#[test]
fn a1e_every_illegal_table_row_has_a_named_trap_and_hint() {
    let mut seen = 0usize;
    for row in TRUTH_TABLE.iter() {
        if row.legal {
            continue;
        }
        let report = report_from_decision("r", row.op, row.prev_mask, 3)
            .unwrap_or_else(|| panic!("row {:?} {:?} must report", row.prev_mask, row.op));
        assert_eq!(report.trap, row.trap.unwrap());
        assert!(!report.trap_name().is_empty());
        assert!(!report.hint.is_empty());
        seen += 1;
    }
    assert!(seen >= 7, "expected all illegal canonical rows to be covered");
}
