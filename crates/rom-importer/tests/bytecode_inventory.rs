use rom_importer::{ImportOptions, inspect_jar};
use rom_model::{
    BytecodeFeature, ClassificationReason, JarReport, MemberReport, MethodBytecodeReport,
    PortingClassification,
};

#[test]
fn classifies_m1_fixture_methods() {
    let report = inspect_m1_fixture();

    let arithmetic = bytecode(&report, "arithmetic");
    assert_eq!(arithmetic.classification, PortingClassification::Green);
    assert!(arithmetic.has_code);
    assert_eq!(
        arithmetic.reason_codes,
        vec![ClassificationReason::SimpleBytecode]
    );
    assert!(arithmetic.opcode_histogram.contains_key("iadd"));

    let field_access = bytecode(&report, "fieldAccess");
    assert_eq!(field_access.classification, PortingClassification::Green);
    assert!(
        field_access
            .referenced_fields
            .iter()
            .any(|field| field.owner == "m1/BytecodeFeatures" && field.name == "stored")
    );

    let switch_method = bytecode(&report, "choose");
    assert_at_least_yellow(switch_method);
    assert!(
        switch_method
            .features
            .contains(&BytecodeFeature::SwitchInstruction)
    );
    assert!(!switch_method.switches.is_empty());

    let synchronized_method = bytecode(&report, "synchronizedMethod");
    assert_at_least_yellow(synchronized_method);
    assert!(
        synchronized_method
            .features
            .contains(&BytecodeFeature::SynchronizedMethod)
    );

    let synchronized_block = bytecode(&report, "synchronizedBlock");
    assert_at_least_yellow(synchronized_block);
    assert!(
        synchronized_block
            .features
            .contains(&BytecodeFeature::MonitorEnter)
    );
    assert!(
        synchronized_block
            .features
            .contains(&BytecodeFeature::MonitorExit)
    );

    let native = bytecode(&report, "nativeCall");
    assert_eq!(native.classification, PortingClassification::Red);
    assert!(!native.has_code);
    assert!(native.features.contains(&BytecodeFeature::NativeMethod));

    let reflection = bytecode(&report, "reflect");
    assert_eq!(reflection.classification, PortingClassification::Red);
    assert!(
        reflection
            .features
            .contains(&BytecodeFeature::ReflectionApi)
    );

    let class_loader = bytecode(&report, "loadWith");
    assert_eq!(class_loader.classification, PortingClassification::Red);
    assert!(
        class_loader
            .features
            .contains(&BytecodeFeature::CustomClassLoader)
    );

    let unsafe_read = bytecode(&report, "unsafeRead");
    assert_eq!(unsafe_read.classification, PortingClassification::Red);
    assert!(unsafe_read.features.contains(&BytecodeFeature::UnsafeApi));

    let load_library = bytecode(&report, "loadLibrary");
    assert_eq!(load_library.classification, PortingClassification::Red);
    assert!(
        load_library
            .features
            .contains(&BytecodeFeature::NativeLibraryLoading)
    );

    let lambda = bytecode(&report, "lambda");
    assert_at_least_yellow(lambda);
    assert!(lambda.features.contains(&BytecodeFeature::InvokeDynamic));
    assert!(
        lambda
            .features
            .contains(&BytecodeFeature::LambdaMetafactory)
    );

    let proxy = bytecode(&report, "proxy");
    assert_eq!(proxy.classification, PortingClassification::Red);
    assert!(proxy.features.contains(&BytecodeFeature::DynamicProxy));

    let invoke_api = bytecode(&report, "invokeApi");
    assert_at_least_yellow(invoke_api);
    assert!(
        invoke_api
            .features
            .contains(&BytecodeFeature::JavaLangInvoke)
    );

    let runtime_generator = bytecode(&report, "runtimeBytecodeGeneratorMarker");
    assert_eq!(runtime_generator.classification, PortingClassification::Red);
    assert!(
        runtime_generator
            .features
            .contains(&BytecodeFeature::RuntimeBytecodeGeneration)
    );

    let catch_one = bytecode(&report, "catchOne");
    assert_at_least_yellow(catch_one);
    assert!(
        catch_one
            .features
            .contains(&BytecodeFeature::ExceptionHandlers)
    );
    assert_eq!(catch_one.exception_handler_count, 1);

    let array_sum = bytecode(&report, "arraySum");
    assert_at_least_yellow(array_sum);
    assert!(
        array_sum
            .features
            .contains(&BytecodeFeature::ArrayOperation)
    );
}

#[test]
fn bytecode_output_ordering_is_deterministic() {
    let first = inspect_m1_fixture();
    let second = inspect_m1_fixture();

    let first_json = serde_json::to_string_pretty(&first).expect("report should serialize");
    let second_json = serde_json::to_string_pretty(&second).expect("report should serialize");

    assert_eq!(first_json, second_json);
}

#[test]
fn malformed_class_does_not_stop_valid_bytecode_inventory() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/malformed/malformed-with-valid.jar"
    );
    let report = inspect_jar(
        path,
        &ImportOptions {
            bytecode: true,
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
}

fn inspect_m1_fixture() -> JarReport {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/m1-bytecode/out/m1-bytecode.jar"
    );
    inspect_jar(
        path,
        &ImportOptions {
            bytecode: true,
            ..ImportOptions::default()
        },
    )
    .expect("M1 bytecode fixture should parse")
}

fn bytecode<'a>(report: &'a JarReport, method_name: &str) -> &'a MethodBytecodeReport {
    method(report, method_name)
        .bytecode
        .as_ref()
        .expect("method should include bytecode report")
}

fn method<'a>(report: &'a JarReport, method_name: &str) -> &'a MemberReport {
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
}

fn assert_at_least_yellow(report: &MethodBytecodeReport) {
    assert_ne!(report.classification, PortingClassification::Green);
}
