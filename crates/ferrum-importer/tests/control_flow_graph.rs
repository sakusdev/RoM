use ferrum_importer::{ImportOptions, inspect_jar};
use ferrum_model::{ControlFlowGraphReport, JarReport, MethodBytecodeReport, TerminatorReport};

#[test]
fn builds_cfg_for_straight_line_method() {
    let report = inspect_m1_fixture();
    let cfg = cfg(&report, "arithmetic");

    assert!(
        cfg.errors.is_empty(),
        "unexpected CFG errors: {:?}",
        cfg.errors
    );
    assert_eq!(cfg.owner, "m1/BytecodeFeatures");
    assert_eq!(cfg.method, "arithmetic");
    assert_eq!(cfg.blocks.len(), 1);
    assert_eq!(cfg.blocks[0].id, 0);
    assert_eq!(cfg.blocks[0].bytecode_start, 0);
    assert!(matches!(cfg.blocks[0].terminator, TerminatorReport::Return));
    assert!(cfg.dot.contains("digraph ferrum_cfg"));
}

#[test]
fn builds_cfg_for_switch_and_loop_methods() {
    let report = inspect_m1_fixture();

    let switch_cfg = cfg(&report, "choose");
    assert!(switch_cfg.errors.is_empty());
    assert!(
        switch_cfg
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, TerminatorReport::Switch { .. }))
    );

    let loop_cfg = cfg(&report, "arraySum");
    assert!(loop_cfg.errors.is_empty());
    assert!(
        loop_cfg
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, TerminatorReport::Branch { .. }))
    );
    assert!(
        loop_cfg
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, TerminatorReport::Goto { .. }))
    );
}

#[test]
fn represents_exception_handler_edges() {
    let report = inspect_m1_fixture();
    let cfg = cfg(&report, "catchOne");

    assert!(cfg.errors.is_empty());
    assert_eq!(cfg.exception_handlers.len(), 1);
    let handler = &cfg.exception_handlers[0];
    assert_eq!(
        handler.catch_type.as_deref(),
        Some("java/lang/NumberFormatException")
    );
    assert!(!handler.covered_blocks.is_empty());

    for covered_block in &handler.covered_blocks {
        let block = cfg
            .blocks
            .iter()
            .find(|block| block.id == *covered_block)
            .expect("covered block should exist");
        assert!(block.successors.contains(&handler.handler_block));
    }
}

#[test]
fn cfg_output_ordering_is_deterministic() {
    let first = inspect_m1_fixture();
    let second = inspect_m1_fixture();

    let first_json = serde_json::to_string_pretty(&first).expect("report should serialize");
    let second_json = serde_json::to_string_pretty(&second).expect("report should serialize");

    assert_eq!(first_json, second_json);
}

#[test]
fn malformed_class_does_not_stop_valid_cfg_inventory() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/malformed/malformed-with-valid.jar"
    );
    let report = inspect_jar(
        path,
        &ImportOptions {
            cfg: true,
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
    assert!(cfg(&report, "choose").errors.is_empty());
}

fn inspect_m1_fixture() -> JarReport {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/m1-bytecode/out/m1-bytecode.jar"
    );
    inspect_jar(
        path,
        &ImportOptions {
            cfg: true,
            ..ImportOptions::default()
        },
    )
    .expect("M1 bytecode fixture should parse")
}

fn cfg<'a>(report: &'a JarReport, method_name: &str) -> &'a ControlFlowGraphReport {
    bytecode(report, method_name)
        .cfg
        .as_ref()
        .expect("method should include CFG")
}

fn bytecode<'a>(report: &'a JarReport, method_name: &str) -> &'a MethodBytecodeReport {
    let class = report
        .classes
        .iter()
        .find(|class| class.internal_name == "m1/BytecodeFeatures")
        .expect("fixture class should be present");

    class
        .methods
        .iter()
        .find(|method| method.name == method_name)
        .unwrap_or_else(|| panic!("fixture method {method_name} should be present"))
        .bytecode
        .as_ref()
        .expect("method should include bytecode")
}
