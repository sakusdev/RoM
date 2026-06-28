use ferrum_importer::{ImportOptions, inspect_jar};
use ferrum_model::{
    IrErrorCode, IrInstructionKindReport, JarReport, JavaTypeReport, MethodIrReport,
};

#[test]
fn builds_typed_ir_for_straight_line_method() {
    let report = inspect_m1_fixture();
    let ir = ir(&report, "arithmetic");

    assert_eq!(
        ir.parsed_descriptor.parameters,
        vec![JavaTypeReport::Int, JavaTypeReport::Int]
    );
    assert_eq!(ir.parsed_descriptor.return_type, JavaTypeReport::Int);
    assert!(ir.locals.iter().any(|local| local.slot == 1
        && local.name.as_deref() == Some("left")
        && local.ty == JavaTypeReport::Int));
    assert!(
        ir.instructions.iter().all(
            |instruction| instruction.source.class == "m1/BytecodeFeatures"
                && instruction.source.method == "arithmetic"
                && instruction.source.descriptor == "(II)I"
        )
    );
    assert!(ir.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        IrInstructionKindReport::LoadLocal { local: 1, .. }
    )));
    assert!(ir.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        IrInstructionKindReport::Binary { operation, .. } if operation == "iadd"
    )));
    assert!(ir.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        IrInstructionKindReport::Return { value: Some(_) }
    )));
    assert!(
        ir.errors.is_empty(),
        "unexpected IR errors: {:?}",
        ir.errors
    );
}

#[test]
fn retains_stack_maps_and_conservative_merge_values() {
    let report = inspect_m1_fixture();
    let choose = ir(&report, "choose");
    let array_sum = ir(&report, "arraySum");

    assert!(
        !choose.stack_map_frames.is_empty(),
        "switch fixture should retain StackMapTable frames"
    );
    assert!(
        !array_sum.merge_values.is_empty(),
        "loop fixture should receive a conservative phi placeholder"
    );
    assert!(
        array_sum.errors.iter().any(|error| {
            error.code == IrErrorCode::ConservativeMerge && error.offset.is_some()
        })
    );
}

#[test]
fn preserves_exception_semantics_as_ir_metadata() {
    let report = inspect_m1_fixture();
    let ir = ir(&report, "catchOne");

    assert_eq!(ir.exception_handlers.len(), 1);
    assert_eq!(
        ir.exception_handlers[0].catch_type.as_deref(),
        Some("java/lang/NumberFormatException")
    );
    assert!(ir.errors.iter().any(|error| {
        error.code == IrErrorCode::ExceptionEdgePreserved && error.offset.is_none()
    }));
}

#[test]
fn ir_output_ordering_is_deterministic() {
    let first = inspect_m1_fixture();
    let second = inspect_m1_fixture();

    let first_json = serde_json::to_string_pretty(&first).expect("report should serialize");
    let second_json = serde_json::to_string_pretty(&second).expect("report should serialize");

    assert_eq!(first_json, second_json);
}

#[test]
fn malformed_class_does_not_stop_valid_ir_inventory() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/malformed/malformed-with-valid.jar"
    );
    let report = inspect_jar(
        path,
        &ImportOptions {
            ir: true,
            ..ImportOptions::default()
        },
    )
    .expect("malformed fixture JAR should be readable");

    assert!(
        report
            .classes
            .iter()
            .any(|class| class.internal_name == "m1/BytecodeFeatures")
    );
    assert_eq!(report.summary.classes_parsed, 1);
    assert_eq!(report.summary.classes_failed, 1);
    assert_eq!(report.errors.len(), 1);
    assert!(ir(&report, "arithmetic").errors.is_empty());
}

fn inspect_m1_fixture() -> JarReport {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/m1-bytecode/out/m1-bytecode.jar"
    );
    inspect_jar(
        path,
        &ImportOptions {
            ir: true,
            ..ImportOptions::default()
        },
    )
    .expect("M1 bytecode fixture should parse")
}

fn ir<'a>(report: &'a JarReport, method_name: &str) -> &'a MethodIrReport {
    report
        .classes
        .iter()
        .find(|class| class.internal_name == "m1/BytecodeFeatures")
        .expect("fixture class should be present")
        .methods
        .iter()
        .find(|method| method.name == method_name)
        .unwrap_or_else(|| panic!("fixture method {method_name} should be present"))
        .bytecode
        .as_ref()
        .expect("method should include bytecode")
        .ir
        .as_ref()
        .expect("method should include typed IR")
}
