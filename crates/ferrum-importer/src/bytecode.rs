use ferrum_model::{
    BasicBlockReport, BranchInstructionReport, BytecodeFeature, CfgErrorCode, CfgErrorReport,
    CfgExceptionHandlerReport, ClassificationReason, ConstantValueReport, ControlFlowGraphReport,
    ExceptionHandlerReport, InstructionReport, InvocationKindReport, IrErrorCode, IrErrorReport,
    IrInstructionKindReport, IrInstructionReport, IrLocalReport, IrLocalSource, IrMergeValueReport,
    IrSourceReport, IrValueReport, JavaTypeReport, LineNumberReport, LocalVariableReport,
    MemberReference, MethodBytecodeReport, MethodIrReport, MonitorOperationReport,
    PortingClassification, ReturnInstructionReport, StackMapFrameReport,
    StackMapVerificationTypeReport, SwitchBlockCaseReport, SwitchCaseReport,
    SwitchInstructionReport, TerminatorReport, ThrowInstructionReport, parse_field_descriptor,
    parse_method_descriptor,
};
use std::collections::{BTreeMap, BTreeSet};

const ACC_STATIC: u16 = 0x0008;
const ACC_SYNCHRONIZED: u16 = 0x0020;
const ACC_NATIVE: u16 = 0x0100;

pub fn inspect_method_bytecode(
    bytes: &[u8],
    include_cfg: bool,
    include_ir: bool,
) -> Result<Vec<MethodBytecodeReport>, String> {
    let class = parse_class(bytes)?;
    class
        .methods
        .iter()
        .map(|method| {
            analyze_method(
                &class.owner,
                method,
                &class.constant_pool,
                &class.bootstrap_methods,
                include_cfg,
                include_ir,
            )
        })
        .collect()
}

#[derive(Debug)]
struct ParsedClass {
    owner: String,
    constant_pool: ConstantPoolData,
    bootstrap_methods: Vec<BootstrapMethod>,
    methods: Vec<RawMethod>,
}

#[derive(Debug)]
struct RawMethod {
    name: String,
    descriptor: String,
    access_flags: u16,
    code: Option<CodeAttribute>,
}

#[derive(Debug)]
struct CodeAttribute {
    max_stack: u16,
    max_locals: u16,
    code: Vec<u8>,
    exception_table: Vec<RawExceptionHandler>,
    line_number_table: Vec<LineNumberReport>,
    local_variable_table: Vec<LocalVariableReport>,
    stack_map_frames: Vec<StackMapFrameReport>,
}

#[derive(Debug)]
struct RawExceptionHandler {
    start_pc: u16,
    end_pc: u16,
    handler_pc: u16,
    catch_type: u16,
}

#[derive(Debug)]
struct BootstrapMethod {
    method_ref: u16,
    arguments: Vec<u16>,
}

#[derive(Debug, Clone)]
enum ConstantPoolEntry {
    Empty,
    Utf8(String),
    Class {
        name_index: u16,
    },
    String {
        string_index: u16,
    },
    FieldRef {
        class_index: u16,
        name_and_type_index: u16,
    },
    MethodRef {
        class_index: u16,
        name_and_type_index: u16,
    },
    InterfaceMethodRef {
        class_index: u16,
        name_and_type_index: u16,
    },
    NameAndType {
        name_index: u16,
        descriptor_index: u16,
    },
    MethodHandle {
        reference_index: u16,
    },
    MethodType {
        descriptor_index: u16,
    },
    Dynamic {
        bootstrap_method_attr_index: u16,
        name_and_type_index: u16,
    },
    InvokeDynamic {
        bootstrap_method_attr_index: u16,
        name_and_type_index: u16,
    },
    Module,
    Package,
}

#[derive(Debug)]
struct ConstantPoolData {
    entries: Vec<ConstantPoolEntry>,
}

impl ConstantPoolData {
    fn parse(reader: &mut Reader<'_>) -> Result<Self, String> {
        let count = reader.read_u16()? as usize;
        let mut entries = Vec::with_capacity(count);
        entries.push(ConstantPoolEntry::Empty);

        let mut index = 1usize;
        while index < count {
            let tag = reader.read_u8()?;
            match tag {
                1 => {
                    let length = reader.read_u16()? as usize;
                    let bytes = reader.read_bytes(length)?;
                    let value = String::from_utf8_lossy(bytes).into_owned();
                    entries.push(ConstantPoolEntry::Utf8(value));
                }
                3 | 4 => {
                    reader.skip(4)?;
                    entries.push(ConstantPoolEntry::Empty);
                }
                5 | 6 => {
                    reader.skip(8)?;
                    entries.push(ConstantPoolEntry::Empty);
                    entries.push(ConstantPoolEntry::Empty);
                    index += 1;
                }
                7 => entries.push(ConstantPoolEntry::Class {
                    name_index: reader.read_u16()?,
                }),
                8 => entries.push(ConstantPoolEntry::String {
                    string_index: reader.read_u16()?,
                }),
                9 => entries.push(ConstantPoolEntry::FieldRef {
                    class_index: reader.read_u16()?,
                    name_and_type_index: reader.read_u16()?,
                }),
                10 => entries.push(ConstantPoolEntry::MethodRef {
                    class_index: reader.read_u16()?,
                    name_and_type_index: reader.read_u16()?,
                }),
                11 => entries.push(ConstantPoolEntry::InterfaceMethodRef {
                    class_index: reader.read_u16()?,
                    name_and_type_index: reader.read_u16()?,
                }),
                12 => entries.push(ConstantPoolEntry::NameAndType {
                    name_index: reader.read_u16()?,
                    descriptor_index: reader.read_u16()?,
                }),
                15 => {
                    reader.read_u8()?;
                    entries.push(ConstantPoolEntry::MethodHandle {
                        reference_index: reader.read_u16()?,
                    });
                }
                16 => entries.push(ConstantPoolEntry::MethodType {
                    descriptor_index: reader.read_u16()?,
                }),
                17 => {
                    let bootstrap_method_attr_index = reader.read_u16()?;
                    entries.push(ConstantPoolEntry::Dynamic {
                        bootstrap_method_attr_index,
                        name_and_type_index: reader.read_u16()?,
                    });
                }
                18 => {
                    let bootstrap_method_attr_index = reader.read_u16()?;
                    entries.push(ConstantPoolEntry::InvokeDynamic {
                        bootstrap_method_attr_index,
                        name_and_type_index: reader.read_u16()?,
                    });
                }
                19 => {
                    reader.read_u16()?;
                    entries.push(ConstantPoolEntry::Module);
                }
                20 => {
                    reader.read_u16()?;
                    entries.push(ConstantPoolEntry::Package);
                }
                _ => return Err(format!("unsupported constant-pool tag {tag} at #{index}")),
            }
            index += 1;
        }

        Ok(Self { entries })
    }

    fn get(&self, index: u16) -> Result<&ConstantPoolEntry, String> {
        self.entries
            .get(index as usize)
            .ok_or_else(|| format!("constant-pool index #{index} is out of bounds"))
    }

    fn utf8(&self, index: u16) -> Result<String, String> {
        match self.get(index)? {
            ConstantPoolEntry::Utf8(value) => Ok(value.clone()),
            _ => Err(format!("constant-pool index #{index} is not a UTF-8 entry")),
        }
    }

    fn class_name(&self, index: u16) -> Result<String, String> {
        match self.get(index)? {
            ConstantPoolEntry::Class { name_index } => self.utf8(*name_index),
            _ => Err(format!("constant-pool index #{index} is not a class entry")),
        }
    }

    fn string_constant(&self, index: u16) -> Result<Option<String>, String> {
        match self.get(index)? {
            ConstantPoolEntry::String { string_index } => self.utf8(*string_index).map(Some),
            ConstantPoolEntry::Utf8(value) => Ok(Some(value.clone())),
            _ => Ok(None),
        }
    }

    fn method_type_descriptor(&self, index: u16) -> Result<Option<String>, String> {
        match self.get(index)? {
            ConstantPoolEntry::MethodType { descriptor_index } => {
                self.utf8(*descriptor_index).map(Some)
            }
            _ => Ok(None),
        }
    }

    fn member_reference(&self, index: u16) -> Result<Option<MemberReference>, String> {
        match self.get(index)? {
            ConstantPoolEntry::FieldRef {
                class_index,
                name_and_type_index,
            }
            | ConstantPoolEntry::MethodRef {
                class_index,
                name_and_type_index,
            }
            | ConstantPoolEntry::InterfaceMethodRef {
                class_index,
                name_and_type_index,
            } => {
                let owner = self.class_name(*class_index)?;
                let (name, descriptor) = self.name_and_type(*name_and_type_index)?;
                Ok(Some(MemberReference {
                    owner,
                    name,
                    descriptor,
                }))
            }
            ConstantPoolEntry::InvokeDynamic {
                bootstrap_method_attr_index: _,
                name_and_type_index,
            }
            | ConstantPoolEntry::Dynamic {
                bootstrap_method_attr_index: _,
                name_and_type_index,
            } => {
                let (name, descriptor) = self.name_and_type(*name_and_type_index)?;
                Ok(Some(MemberReference {
                    owner: "<invokedynamic>".to_owned(),
                    name,
                    descriptor,
                }))
            }
            ConstantPoolEntry::MethodHandle { reference_index } => {
                self.member_reference(*reference_index)
            }
            _ => Ok(None),
        }
    }

    fn name_and_type(&self, index: u16) -> Result<(String, String), String> {
        match self.get(index)? {
            ConstantPoolEntry::NameAndType {
                name_index,
                descriptor_index,
            } => Ok((self.utf8(*name_index)?, self.utf8(*descriptor_index)?)),
            _ => Err(format!(
                "constant-pool index #{index} is not a name-and-type entry"
            )),
        }
    }

    fn bootstrap_method_attr_index(&self, index: u16) -> Result<Option<u16>, String> {
        match self.get(index)? {
            ConstantPoolEntry::InvokeDynamic {
                bootstrap_method_attr_index,
                name_and_type_index: _,
            }
            | ConstantPoolEntry::Dynamic {
                bootstrap_method_attr_index,
                name_and_type_index: _,
            } => Ok(Some(*bootstrap_method_attr_index)),
            _ => Ok(None),
        }
    }
}

fn parse_class(bytes: &[u8]) -> Result<ParsedClass, String> {
    let mut reader = Reader::new(bytes);
    let magic = reader.read_u32()?;
    if magic != 0xCAFEBABE {
        return Err("class file does not start with CAFEBABE".to_owned());
    }

    reader.read_u16()?;
    reader.read_u16()?;
    let constant_pool = ConstantPoolData::parse(&mut reader)?;

    reader.read_u16()?;
    let this_class = reader.read_u16()?;
    reader.read_u16()?;
    let owner = constant_pool.class_name(this_class)?;

    let interfaces_count = reader.read_u16()? as usize;
    reader.skip(interfaces_count * 2)?;

    skip_members(&mut reader)?;
    let methods = parse_methods(&mut reader, &constant_pool)?;
    let bootstrap_methods = parse_class_attributes(&mut reader, &constant_pool)?;

    Ok(ParsedClass {
        owner,
        constant_pool,
        bootstrap_methods,
        methods,
    })
}

fn skip_members(reader: &mut Reader<'_>) -> Result<(), String> {
    let count = reader.read_u16()? as usize;
    for _ in 0..count {
        reader.read_u16()?;
        reader.read_u16()?;
        reader.read_u16()?;
        skip_attributes(reader)?;
    }
    Ok(())
}

fn skip_attributes(reader: &mut Reader<'_>) -> Result<(), String> {
    let count = reader.read_u16()? as usize;
    for _ in 0..count {
        reader.read_u16()?;
        let length = reader.read_u32()? as usize;
        reader.skip(length)?;
    }
    Ok(())
}

fn parse_class_attributes(
    reader: &mut Reader<'_>,
    constant_pool: &ConstantPoolData,
) -> Result<Vec<BootstrapMethod>, String> {
    let count = reader.read_u16()? as usize;
    let mut bootstrap_methods = Vec::new();
    for _ in 0..count {
        let name_index = reader.read_u16()?;
        let length = reader.read_u32()? as usize;
        let attribute_bytes = reader.read_bytes(length)?;
        if constant_pool.utf8(name_index)? == "BootstrapMethods" {
            bootstrap_methods = parse_bootstrap_methods(attribute_bytes)?;
        }
    }
    Ok(bootstrap_methods)
}

fn parse_bootstrap_methods(bytes: &[u8]) -> Result<Vec<BootstrapMethod>, String> {
    let mut reader = Reader::new(bytes);
    let count = reader.read_u16()? as usize;
    let mut methods = Vec::with_capacity(count);
    for _ in 0..count {
        let method_ref = reader.read_u16()?;
        let argument_count = reader.read_u16()? as usize;
        let mut arguments = Vec::with_capacity(argument_count);
        for _ in 0..argument_count {
            arguments.push(reader.read_u16()?);
        }
        methods.push(BootstrapMethod {
            method_ref,
            arguments,
        });
    }
    Ok(methods)
}

fn parse_methods(
    reader: &mut Reader<'_>,
    constant_pool: &ConstantPoolData,
) -> Result<Vec<RawMethod>, String> {
    let count = reader.read_u16()? as usize;
    let mut methods = Vec::with_capacity(count);

    for _ in 0..count {
        let access_flags = reader.read_u16()?;
        let name = constant_pool.utf8(reader.read_u16()?)?;
        let descriptor = constant_pool.utf8(reader.read_u16()?)?;

        let attributes_count = reader.read_u16()? as usize;
        let mut code = None;
        for _ in 0..attributes_count {
            let name_index = reader.read_u16()?;
            let length = reader.read_u32()? as usize;
            let attribute_bytes = reader.read_bytes(length)?;
            if constant_pool.utf8(name_index)? == "Code" {
                code = Some(parse_code_attribute(attribute_bytes, constant_pool)?);
            }
        }

        methods.push(RawMethod {
            name,
            descriptor,
            access_flags,
            code,
        });
    }

    Ok(methods)
}

fn parse_code_attribute(
    bytes: &[u8],
    constant_pool: &ConstantPoolData,
) -> Result<CodeAttribute, String> {
    let mut reader = Reader::new(bytes);
    let max_stack = reader.read_u16()?;
    let max_locals = reader.read_u16()?;
    let code_length = reader.read_u32()? as usize;
    let code = reader.read_bytes(code_length)?.to_vec();

    let exception_count = reader.read_u16()? as usize;
    let mut exception_table = Vec::with_capacity(exception_count);
    for _ in 0..exception_count {
        exception_table.push(RawExceptionHandler {
            start_pc: reader.read_u16()?,
            end_pc: reader.read_u16()?,
            handler_pc: reader.read_u16()?,
            catch_type: reader.read_u16()?,
        });
    }

    let attributes_count = reader.read_u16()? as usize;
    let mut line_number_table = Vec::new();
    let mut local_variable_table = Vec::new();
    let mut stack_map_frames = Vec::new();
    for _ in 0..attributes_count {
        let name_index = reader.read_u16()?;
        let length = reader.read_u32()? as usize;
        let attribute_bytes = reader.read_bytes(length)?;
        match constant_pool.utf8(name_index)?.as_str() {
            "LineNumberTable" => {
                line_number_table.extend(parse_line_number_table(attribute_bytes)?);
            }
            "LocalVariableTable" => {
                local_variable_table
                    .extend(parse_local_variable_table(attribute_bytes, constant_pool)?);
            }
            "StackMapTable" => {
                stack_map_frames.extend(parse_stack_map_table(attribute_bytes, constant_pool)?);
            }
            _ => {}
        }
    }

    Ok(CodeAttribute {
        max_stack,
        max_locals,
        code,
        exception_table,
        line_number_table,
        local_variable_table,
        stack_map_frames,
    })
}

fn parse_line_number_table(bytes: &[u8]) -> Result<Vec<LineNumberReport>, String> {
    let mut reader = Reader::new(bytes);
    let count = reader.read_u16()? as usize;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        entries.push(LineNumberReport {
            start_pc: reader.read_u16()?,
            line_number: reader.read_u16()?,
        });
    }
    Ok(entries)
}

fn parse_local_variable_table(
    bytes: &[u8],
    constant_pool: &ConstantPoolData,
) -> Result<Vec<LocalVariableReport>, String> {
    let mut reader = Reader::new(bytes);
    let count = reader.read_u16()? as usize;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let start_pc = reader.read_u16()?;
        let length = reader.read_u16()?;
        let name = constant_pool.utf8(reader.read_u16()?)?;
        let descriptor = constant_pool.utf8(reader.read_u16()?)?;
        let index = reader.read_u16()?;
        entries.push(LocalVariableReport {
            start_pc,
            length,
            index,
            name,
            descriptor,
        });
    }
    Ok(entries)
}

fn parse_stack_map_table(
    bytes: &[u8],
    constant_pool: &ConstantPoolData,
) -> Result<Vec<StackMapFrameReport>, String> {
    let mut reader = Reader::new(bytes);
    let count = reader.read_u16()?;
    let mut frames = Vec::with_capacity(count as usize);
    let mut previous_offset: Option<u32> = None;

    for index in 0..count {
        let frame_type = reader.read_u8()?;
        let (kind, offset_delta, locals, stack) = match frame_type {
            0..=63 => ("same".to_owned(), frame_type as u16, Vec::new(), Vec::new()),
            64..=127 => (
                "same_locals_1_stack_item".to_owned(),
                (frame_type - 64) as u16,
                Vec::new(),
                vec![parse_verification_type(&mut reader, constant_pool)?],
            ),
            247 => {
                let offset_delta = reader.read_u16()?;
                (
                    "same_locals_1_stack_item_extended".to_owned(),
                    offset_delta,
                    Vec::new(),
                    vec![parse_verification_type(&mut reader, constant_pool)?],
                )
            }
            248..=250 => (
                "chop".to_owned(),
                reader.read_u16()?,
                Vec::new(),
                Vec::new(),
            ),
            251 => (
                "same_extended".to_owned(),
                reader.read_u16()?,
                Vec::new(),
                Vec::new(),
            ),
            252..=254 => {
                let offset_delta = reader.read_u16()?;
                let local_count = (frame_type - 251) as usize;
                let mut locals = Vec::with_capacity(local_count);
                for _ in 0..local_count {
                    locals.push(parse_verification_type(&mut reader, constant_pool)?);
                }
                ("append".to_owned(), offset_delta, locals, Vec::new())
            }
            255 => {
                let offset_delta = reader.read_u16()?;
                let local_count = reader.read_u16()? as usize;
                let mut locals = Vec::with_capacity(local_count);
                for _ in 0..local_count {
                    locals.push(parse_verification_type(&mut reader, constant_pool)?);
                }
                let stack_count = reader.read_u16()? as usize;
                let mut stack = Vec::with_capacity(stack_count);
                for _ in 0..stack_count {
                    stack.push(parse_verification_type(&mut reader, constant_pool)?);
                }
                ("full".to_owned(), offset_delta, locals, stack)
            }
            _ => return Err(format!("invalid StackMapTable frame type {frame_type}")),
        };

        let bytecode_offset = match previous_offset {
            Some(previous) => previous
                .checked_add(offset_delta as u32)
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| "StackMapTable bytecode offset overflowed".to_owned())?,
            None => offset_delta as u32,
        };
        previous_offset = Some(bytecode_offset);
        frames.push(StackMapFrameReport {
            index,
            frame_type,
            kind,
            offset_delta,
            bytecode_offset,
            locals,
            stack,
        });
    }

    Ok(frames)
}

fn parse_verification_type(
    reader: &mut Reader<'_>,
    constant_pool: &ConstantPoolData,
) -> Result<StackMapVerificationTypeReport, String> {
    match reader.read_u8()? {
        0 => Ok(StackMapVerificationTypeReport::Top),
        1 => Ok(StackMapVerificationTypeReport::Integer),
        2 => Ok(StackMapVerificationTypeReport::Float),
        3 => Ok(StackMapVerificationTypeReport::Double),
        4 => Ok(StackMapVerificationTypeReport::Long),
        5 => Ok(StackMapVerificationTypeReport::Null),
        6 => Ok(StackMapVerificationTypeReport::UninitializedThis),
        7 => Ok(StackMapVerificationTypeReport::Object {
            class: constant_pool.class_name(reader.read_u16()?)?,
        }),
        8 => Ok(StackMapVerificationTypeReport::Uninitialized {
            offset: reader.read_u16()?,
        }),
        tag => Err(format!("invalid StackMapTable verification type {tag}")),
    }
}

fn analyze_method(
    owner: &str,
    method: &RawMethod,
    constant_pool: &ConstantPoolData,
    bootstrap_methods: &[BootstrapMethod],
    include_cfg: bool,
    include_ir: bool,
) -> Result<MethodBytecodeReport, String> {
    let mut features = BTreeSet::new();
    let mut reason_codes = BTreeSet::new();

    if method.access_flags & ACC_NATIVE != 0 {
        features.insert(BytecodeFeature::NativeMethod);
        reason_codes.insert(ClassificationReason::NativeMethod);
    }
    if method.access_flags & ACC_SYNCHRONIZED != 0 {
        features.insert(BytecodeFeature::SynchronizedMethod);
        reason_codes.insert(ClassificationReason::SynchronizedMethod);
    }

    let Some(code) = &method.code else {
        if reason_codes.is_empty() {
            reason_codes.insert(ClassificationReason::NoBytecodeBody);
        }
        let classification = classify(&reason_codes);
        return Ok(MethodBytecodeReport {
            has_code: false,
            code_length: None,
            max_stack: None,
            max_locals: None,
            exception_handler_count: 0,
            exception_table: Vec::new(),
            line_number_table: Vec::new(),
            local_variable_table: Vec::new(),
            instruction_count: 0,
            instructions: Vec::new(),
            opcode_histogram: BTreeMap::new(),
            branches: Vec::new(),
            switches: Vec::new(),
            returns: Vec::new(),
            throws: Vec::new(),
            referenced_methods: Vec::new(),
            referenced_fields: Vec::new(),
            referenced_types: Vec::new(),
            string_constants: Vec::new(),
            features: features.into_iter().collect(),
            classification,
            reason_codes: reason_codes.iter().copied().collect(),
            reasons: reason_codes
                .iter()
                .map(|reason| reason_text(*reason))
                .collect(),
            cfg: None,
            ir: None,
        });
    };

    let mut collector = AnalysisCollector {
        instructions: Vec::new(),
        opcode_histogram: BTreeMap::new(),
        branches: Vec::new(),
        switches: Vec::new(),
        returns: Vec::new(),
        throws: Vec::new(),
        referenced_methods: BTreeSet::new(),
        referenced_fields: BTreeSet::new(),
        referenced_types: BTreeSet::new(),
        string_constants: BTreeSet::new(),
        features,
        reason_codes,
    };

    if !code.exception_table.is_empty() {
        collector
            .features
            .insert(BytecodeFeature::ExceptionHandlers);
        collector
            .reason_codes
            .insert(ClassificationReason::UsesExceptionHandlers);
    }

    let mut exception_table = Vec::with_capacity(code.exception_table.len());
    for handler in &code.exception_table {
        let catch_type = if handler.catch_type == 0 {
            None
        } else {
            let catch_type = constant_pool.class_name(handler.catch_type)?;
            collector.referenced_types.insert(catch_type.clone());
            Some(catch_type)
        };
        exception_table.push(ExceptionHandlerReport {
            start_pc: handler.start_pc,
            end_pc: handler.end_pc,
            handler_pc: handler.handler_pc,
            catch_type,
        });
    }

    for local in &code.local_variable_table {
        collect_descriptor_types(&local.descriptor, &mut collector.referenced_types);
    }

    decode_code(&code.code, constant_pool, bootstrap_methods, &mut collector)?;

    if collector.branches.len() > 8 {
        collector
            .reason_codes
            .insert(ClassificationReason::ComplexBranching);
    }

    if collector.reason_codes.is_empty() {
        collector
            .reason_codes
            .insert(ClassificationReason::SimpleBytecode);
    }

    let classification = classify(&collector.reason_codes);
    let reason_codes: Vec<_> = collector.reason_codes.iter().copied().collect();
    let reasons = reason_codes
        .iter()
        .map(|reason| reason_text(*reason))
        .collect();

    let mut report = MethodBytecodeReport {
        has_code: true,
        code_length: Some(code.code.len() as u32),
        max_stack: Some(code.max_stack),
        max_locals: Some(code.max_locals),
        exception_handler_count: exception_table.len(),
        exception_table,
        line_number_table: code.line_number_table.clone(),
        local_variable_table: code.local_variable_table.clone(),
        instruction_count: collector.instructions.len(),
        instructions: collector.instructions,
        opcode_histogram: collector.opcode_histogram,
        branches: collector.branches,
        switches: collector.switches,
        returns: collector.returns,
        throws: collector.throws,
        referenced_methods: collector.referenced_methods.into_iter().collect(),
        referenced_fields: collector.referenced_fields.into_iter().collect(),
        referenced_types: collector.referenced_types.into_iter().collect(),
        string_constants: collector.string_constants.into_iter().collect(),
        features: collector.features.into_iter().collect(),
        classification,
        reason_codes,
        reasons,
        cfg: None,
        ir: None,
    };

    if include_cfg {
        report.cfg = Some(build_cfg(owner, &method.name, &method.descriptor, &report));
    }
    if include_ir {
        report.ir = Some(build_ir(
            owner,
            method,
            code,
            constant_pool,
            report.cfg.as_ref(),
            &report.exception_table,
        ));
    }

    Ok(report)
}

fn build_ir(
    owner: &str,
    method: &RawMethod,
    code: &CodeAttribute,
    constant_pool: &ConstantPoolData,
    cfg: Option<&ControlFlowGraphReport>,
    exception_table: &[ExceptionHandlerReport],
) -> MethodIrReport {
    let mut errors = Vec::new();
    let parsed_descriptor = match parse_method_descriptor(&method.descriptor) {
        Ok(descriptor) => descriptor,
        Err(error) => {
            errors.push(ir_error(
                None,
                IrErrorCode::DescriptorParseFailed,
                format!(
                    "cannot parse method descriptor {}.{}{}: {error}",
                    owner, method.name, method.descriptor
                ),
            ));
            ferrum_model::MethodDescriptorReport {
                parameters: Vec::new(),
                return_type: JavaTypeReport::Unknown {
                    reason: "invalid_method_descriptor".to_owned(),
                },
            }
        }
    };

    let locals = initial_ir_locals(
        owner,
        method.access_flags,
        code.max_locals,
        &parsed_descriptor,
        &code.local_variable_table,
        &mut errors,
    );
    let mut builder = IrBuilder::new(
        owner,
        &method.name,
        &method.descriptor,
        &code.line_number_table,
        locals,
        exception_handler_types(exception_table),
        errors,
    );
    builder.lower_code(&code.code, constant_pool);

    if !exception_table.is_empty() {
        builder.errors.push(ir_error(
            None,
            IrErrorCode::ExceptionEdgePreserved,
            "exception handlers are preserved as exceptional control-flow metadata".to_owned(),
        ));
    }

    let merge_values = if let Some(cfg) = cfg {
        builder.add_cfg_merges(cfg)
    } else {
        Vec::new()
    };
    builder.instructions.sort_by(|left, right| {
        left.source
            .bytecode_offset
            .cmp(&right.source.bytecode_offset)
            .then(left.id.cmp(&right.id))
    });

    MethodIrReport {
        owner: owner.to_owned(),
        method: method.name.clone(),
        descriptor: method.descriptor.clone(),
        parsed_descriptor,
        locals: builder.locals.into_values().collect(),
        stack_map_frames: code.stack_map_frames.clone(),
        instructions: builder.instructions,
        merge_values,
        exception_handlers: exception_table.to_vec(),
        errors: builder.errors,
    }
}

fn initial_ir_locals(
    owner: &str,
    access_flags: u16,
    max_locals: u16,
    descriptor: &ferrum_model::MethodDescriptorReport,
    local_variable_table: &[LocalVariableReport],
    errors: &mut Vec<IrErrorReport>,
) -> BTreeMap<u16, IrLocalReport> {
    let mut locals = BTreeMap::new();
    let mut slot = 0u16;

    if access_flags & ACC_STATIC == 0 {
        let ty = JavaTypeReport::Object {
            internal_name: owner.to_owned(),
        };
        locals.insert(
            slot,
            IrLocalReport {
                slot,
                name: Some("this".to_owned()),
                slot_width: ty.slot_width(),
                ty,
                source: IrLocalSource::This,
                start_pc: Some(0),
                end_pc: None,
            },
        );
        slot += 1;
    }

    for (index, ty) in descriptor.parameters.iter().enumerate() {
        locals.insert(
            slot,
            IrLocalReport {
                slot,
                name: Some(format!("arg{index}")),
                slot_width: ty.slot_width(),
                ty: ty.clone(),
                source: IrLocalSource::MethodParameter,
                start_pc: Some(0),
                end_pc: None,
            },
        );
        slot = slot.saturating_add(ty.slot_width());
    }

    for local in local_variable_table {
        match parse_field_descriptor(&local.descriptor) {
            Ok(ty) => {
                let slot_width = ty.slot_width();
                locals.insert(
                    local.index,
                    IrLocalReport {
                        slot: local.index,
                        name: Some(local.name.clone()),
                        ty,
                        slot_width,
                        source: IrLocalSource::LocalVariableTable,
                        start_pc: Some(local.start_pc),
                        end_pc: Some(local.start_pc.saturating_add(local.length)),
                    },
                );
            }
            Err(error) => errors.push(ir_error(
                Some(local.start_pc as u32),
                IrErrorCode::DescriptorParseFailed,
                format!(
                    "cannot parse LocalVariableTable descriptor for slot {}: {error}",
                    local.index
                ),
            )),
        }
    }

    for slot in 0..max_locals {
        locals.entry(slot).or_insert_with(|| {
            let ty = JavaTypeReport::Unknown {
                reason: "local_type_not_declared".to_owned(),
            };
            IrLocalReport {
                slot,
                name: None,
                slot_width: ty.slot_width(),
                ty,
                source: IrLocalSource::Unknown,
                start_pc: None,
                end_pc: None,
            }
        });
    }

    locals
}

fn exception_handler_types(
    exception_table: &[ExceptionHandlerReport],
) -> BTreeMap<u32, JavaTypeReport> {
    exception_table
        .iter()
        .map(|handler| {
            let ty = JavaTypeReport::Object {
                internal_name: handler
                    .catch_type
                    .clone()
                    .unwrap_or_else(|| "java/lang/Throwable".to_owned()),
            };
            (u32::from(handler.handler_pc), ty)
        })
        .collect()
}

struct IrBuilder<'a> {
    owner: &'a str,
    method: &'a str,
    descriptor: &'a str,
    line_number_table: &'a [LineNumberReport],
    locals: BTreeMap<u16, IrLocalReport>,
    exception_handler_types: BTreeMap<u32, JavaTypeReport>,
    stack: Vec<StackValue>,
    instructions: Vec<IrInstructionReport>,
    errors: Vec<IrErrorReport>,
    next_instruction_id: u32,
    next_value_id: u32,
}

#[derive(Debug, Clone)]
struct StackValue {
    id: u32,
    ty: JavaTypeReport,
}

impl<'a> IrBuilder<'a> {
    fn new(
        owner: &'a str,
        method: &'a str,
        descriptor: &'a str,
        line_number_table: &'a [LineNumberReport],
        locals: BTreeMap<u16, IrLocalReport>,
        exception_handler_types: BTreeMap<u32, JavaTypeReport>,
        errors: Vec<IrErrorReport>,
    ) -> Self {
        Self {
            owner,
            method,
            descriptor,
            line_number_table,
            locals,
            exception_handler_types,
            stack: Vec::new(),
            instructions: Vec::new(),
            errors,
            next_instruction_id: 0,
            next_value_id: 0,
        }
    }

    fn lower_code(&mut self, code: &[u8], constant_pool: &ConstantPoolData) {
        let mut offset = 0usize;
        while offset < code.len() {
            let opcode = code[offset];
            self.seed_exception_handler_stack(offset as u32);
            let result = self.lower_instruction(code, offset, opcode, constant_pool);
            match result {
                Ok(next_offset) if next_offset > offset => offset = next_offset,
                Ok(_) => {
                    self.errors.push(ir_error(
                        Some(offset as u32),
                        IrErrorCode::UnsupportedOpcode,
                        format!("IR lowering for {} did not advance", opcode_name(opcode)),
                    ));
                    break;
                }
                Err(error) => {
                    self.errors.push(ir_error(
                        Some(offset as u32),
                        IrErrorCode::UnsupportedOpcode,
                        error,
                    ));
                    break;
                }
            }
        }
    }

    fn lower_instruction(
        &mut self,
        code: &[u8],
        offset: usize,
        opcode: u8,
        constant_pool: &ConstantPoolData,
    ) -> Result<usize, String> {
        let opcode_name = opcode_name(opcode).to_owned();
        match opcode {
            0x00 => {
                self.emit(
                    offset,
                    IrInstructionKindReport::StackOperation {
                        operation: "nop".to_owned(),
                        values: Vec::new(),
                    },
                );
                Ok(offset + 1)
            }
            0x01 => {
                let output = self.push_value(JavaTypeReport::Null);
                self.emit(
                    offset,
                    IrInstructionKindReport::Constant {
                        output,
                        value: ConstantValueReport::Null,
                    },
                );
                Ok(offset + 1)
            }
            0x02..=0x08 => {
                let value = i32::from(opcode) - 0x03;
                self.push_int_constant(offset, value);
                Ok(offset + 1)
            }
            0x09 | 0x0a => {
                let value = i64::from(opcode - 0x09);
                let output = self.push_value(JavaTypeReport::Long);
                self.emit(
                    offset,
                    IrInstructionKindReport::Constant {
                        output,
                        value: ConstantValueReport::Long { value },
                    },
                );
                Ok(offset + 1)
            }
            0x0b..=0x0d => {
                let value = f32::from(opcode - 0x0b);
                let output = self.push_value(JavaTypeReport::Float);
                self.emit(
                    offset,
                    IrInstructionKindReport::Constant {
                        output,
                        value: ConstantValueReport::Float { value },
                    },
                );
                Ok(offset + 1)
            }
            0x0e | 0x0f => {
                let value = f64::from(opcode - 0x0e);
                let output = self.push_value(JavaTypeReport::Double);
                self.emit(
                    offset,
                    IrInstructionKindReport::Constant {
                        output,
                        value: ConstantValueReport::Double { value },
                    },
                );
                Ok(offset + 1)
            }
            0x10 => {
                let value = read_u8_at(code, offset + 1)? as i8 as i32;
                self.push_int_constant(offset, value);
                Ok(offset + 2)
            }
            0x11 => {
                let value = read_i16_at(code, offset + 1)? as i32;
                self.push_int_constant(offset, value);
                Ok(offset + 3)
            }
            0x12 => {
                let index = read_u8_at(code, offset + 1)? as u16;
                self.push_ldc(offset, index, constant_pool, false);
                Ok(offset + 2)
            }
            0x13 | 0x14 => {
                let index = read_u16_at(code, offset + 1)?;
                self.push_ldc(offset, index, constant_pool, opcode == 0x14);
                Ok(offset + 3)
            }
            0x15..=0x19 => {
                let local = read_u8_at(code, offset + 1)? as u16;
                self.load_local(offset, local, load_fallback_type(opcode));
                Ok(offset + 2)
            }
            0x1a..=0x2d => {
                let (local, ty) = compact_load(opcode);
                self.load_local(offset, local, ty);
                Ok(offset + 1)
            }
            0x2e..=0x35 => {
                let index = self.pop(offset, "array index");
                let array = self.pop(offset, "array reference");
                let output = self.push_value(array_load_type(opcode));
                self.emit(
                    offset,
                    IrInstructionKindReport::ArrayLoad {
                        output,
                        array: array.id,
                        index: index.id,
                    },
                );
                Ok(offset + 1)
            }
            0x36..=0x3a => {
                let local = read_u8_at(code, offset + 1)? as u16;
                self.store_local(offset, local);
                Ok(offset + 2)
            }
            0x3b..=0x4e => {
                let local = compact_store(opcode);
                self.store_local(offset, local);
                Ok(offset + 1)
            }
            0x4f..=0x56 => {
                let value = self.pop(offset, "array store value");
                let index = self.pop(offset, "array index");
                let array = self.pop(offset, "array reference");
                self.emit(
                    offset,
                    IrInstructionKindReport::ArrayStore {
                        array: array.id,
                        index: index.id,
                        value: value.id,
                    },
                );
                Ok(offset + 1)
            }
            0x57 => {
                let value = self.pop(offset, "pop value");
                self.emit(
                    offset,
                    IrInstructionKindReport::StackOperation {
                        operation: "pop".to_owned(),
                        values: vec![value.id],
                    },
                );
                Ok(offset + 1)
            }
            0x58 => {
                let mut values = Vec::new();
                let first = self.pop(offset, "pop2 value");
                values.push(first.id);
                if first.ty.slot_width() == 1 {
                    values.push(self.pop(offset, "pop2 second value").id);
                }
                self.emit(
                    offset,
                    IrInstructionKindReport::StackOperation {
                        operation: "pop2".to_owned(),
                        values,
                    },
                );
                Ok(offset + 1)
            }
            0x59 => {
                let value = self.peek_or_unknown(offset, "dup value");
                self.stack.push(value.clone());
                self.emit(
                    offset,
                    IrInstructionKindReport::StackOperation {
                        operation: "dup".to_owned(),
                        values: vec![value.id],
                    },
                );
                Ok(offset + 1)
            }
            0x5f => {
                if self.stack.len() < 2 {
                    self.errors.push(ir_error(
                        Some(offset as u32),
                        IrErrorCode::StackUnderflow,
                        "swap requires two stack values".to_owned(),
                    ));
                } else {
                    let len = self.stack.len();
                    self.stack.swap(len - 1, len - 2);
                }
                self.emit(
                    offset,
                    IrInstructionKindReport::StackOperation {
                        operation: "swap".to_owned(),
                        values: Vec::new(),
                    },
                );
                Ok(offset + 1)
            }
            0x5a..=0x5e => self.unsupported_fixed(code, offset, opcode, "complex dup form"),
            0x60..=0x73 | 0x78..=0x83 | 0x94..=0x98 => {
                let right = self.pop(offset, "binary right operand");
                let left = self.pop(offset, "binary left operand");
                let output = self.push_value(binary_output_type(opcode));
                self.emit(
                    offset,
                    IrInstructionKindReport::Binary {
                        output,
                        operation: opcode_name,
                        left: left.id,
                        right: right.id,
                    },
                );
                Ok(offset + 1)
            }
            0x84 => {
                let local = read_u8_at(code, offset + 1)? as u16;
                let amount = read_u8_at(code, offset + 2)? as i8 as i16;
                self.emit(
                    offset,
                    IrInstructionKindReport::LocalIncrement { local, amount },
                );
                self.infer_local(local, JavaTypeReport::Int);
                Ok(offset + 3)
            }
            0x85..=0x93 => {
                let input = self.pop(offset, "conversion input");
                let output = self.push_value(conversion_output_type(opcode));
                self.emit(
                    offset,
                    IrInstructionKindReport::Convert {
                        output,
                        operation: opcode_name,
                        input: input.id,
                    },
                );
                Ok(offset + 1)
            }
            0x74..=0x77 => {
                let input = self.pop(offset, "unary input");
                let output = self.push_value(unary_output_type(opcode));
                self.emit(
                    offset,
                    IrInstructionKindReport::Unary {
                        output,
                        operation: opcode_name,
                        input: input.id,
                    },
                );
                Ok(offset + 1)
            }
            0x99..=0xa6 | 0xc6 | 0xc7 => {
                let relative = read_i16_at(code, offset + 1)? as i32;
                let condition_values = self.branch_condition_values(offset, opcode);
                self.emit(
                    offset,
                    IrInstructionKindReport::Branch {
                        opcode: opcode_name,
                        condition_values,
                        target: branch_target(offset, relative)?,
                        fallthrough: Some((offset + 3) as u32),
                    },
                );
                Ok(offset + 3)
            }
            0xa7 => {
                let relative = read_i16_at(code, offset + 1)? as i32;
                self.emit(
                    offset,
                    IrInstructionKindReport::Branch {
                        opcode: opcode_name,
                        condition_values: Vec::new(),
                        target: branch_target(offset, relative)?,
                        fallthrough: None,
                    },
                );
                Ok(offset + 3)
            }
            0xa8 => Ok(self.unsupported_variable_length(offset, opcode, 3, "legacy subroutine")),
            0xa9 => Ok(self.unsupported_variable_length(offset, opcode, 2, "legacy subroutine")),
            0xaa => self.lower_table_switch(code, offset),
            0xab => self.lower_lookup_switch(code, offset),
            0xac..=0xb0 => {
                let value = self.pop(offset, "return value");
                self.emit(
                    offset,
                    IrInstructionKindReport::Return {
                        value: Some(value.id),
                    },
                );
                Ok(offset + 1)
            }
            0xb1 => {
                self.emit(offset, IrInstructionKindReport::Return { value: None });
                Ok(offset + 1)
            }
            0xb2..=0xb5 => {
                let index = read_u16_at(code, offset + 1)?;
                let field = self.member_reference(offset, index, constant_pool)?;
                let field_type =
                    parse_field_descriptor(&field.descriptor).unwrap_or_else(|error| {
                        self.errors.push(ir_error(
                            Some(offset as u32),
                            IrErrorCode::DescriptorParseFailed,
                            format!(
                                "cannot parse field descriptor {}.{}{}: {error}",
                                field.owner, field.name, field.descriptor
                            ),
                        ));
                        JavaTypeReport::Unknown {
                            reason: "invalid_field_descriptor".to_owned(),
                        }
                    });
                match opcode {
                    0xb2 => {
                        let output = self.push_value(field_type);
                        self.emit(
                            offset,
                            IrInstructionKindReport::LoadField {
                                output,
                                object: None,
                                field,
                            },
                        );
                    }
                    0xb3 => {
                        let value = self.pop(offset, "static field value");
                        self.emit(
                            offset,
                            IrInstructionKindReport::StoreField {
                                object: None,
                                field,
                                value: value.id,
                            },
                        );
                    }
                    0xb4 => {
                        let object = self.pop(offset, "field receiver");
                        let output = self.push_value(field_type);
                        self.emit(
                            offset,
                            IrInstructionKindReport::LoadField {
                                output,
                                object: Some(object.id),
                                field,
                            },
                        );
                    }
                    _ => {
                        let value = self.pop(offset, "field value");
                        let object = self.pop(offset, "field receiver");
                        self.emit(
                            offset,
                            IrInstructionKindReport::StoreField {
                                object: Some(object.id),
                                field,
                                value: value.id,
                            },
                        );
                    }
                }
                Ok(offset + 3)
            }
            0xb6..=0xba => self.lower_invoke(code, offset, opcode, constant_pool),
            0xbb => {
                let index = read_u16_at(code, offset + 1)?;
                let class = constant_pool.class_name(index)?;
                let output = self.push_value(JavaTypeReport::Object {
                    internal_name: class.clone(),
                });
                self.emit(offset, IrInstructionKindReport::NewObject { output, class });
                Ok(offset + 3)
            }
            0xbc => {
                let atype = read_u8_at(code, offset + 1)?;
                let count = self.pop(offset, "newarray length");
                let component_type = primitive_array_type(atype);
                let output = self.push_value(JavaTypeReport::Array {
                    dimensions: 1,
                    component: Box::new(component_type.clone()),
                });
                self.emit(
                    offset,
                    IrInstructionKindReport::NewArray {
                        output,
                        component_type,
                        dimensions: vec![count.id],
                    },
                );
                Ok(offset + 2)
            }
            0xbd => {
                let index = read_u16_at(code, offset + 1)?;
                let count = self.pop(offset, "anewarray length");
                let component_type = JavaTypeReport::Object {
                    internal_name: constant_pool.class_name(index)?,
                };
                let output = self.push_value(JavaTypeReport::Array {
                    dimensions: 1,
                    component: Box::new(component_type.clone()),
                });
                self.emit(
                    offset,
                    IrInstructionKindReport::NewArray {
                        output,
                        component_type,
                        dimensions: vec![count.id],
                    },
                );
                Ok(offset + 3)
            }
            0xbe => {
                let array = self.pop(offset, "arraylength receiver");
                let output = self.push_value(JavaTypeReport::Int);
                self.emit(
                    offset,
                    IrInstructionKindReport::ArrayLength {
                        output,
                        array: array.id,
                    },
                );
                Ok(offset + 1)
            }
            0xbf => {
                let value = self.pop(offset, "throw value");
                self.emit(offset, IrInstructionKindReport::Throw { value: value.id });
                Ok(offset + 1)
            }
            0xc0 | 0xc1 => {
                let index = read_u16_at(code, offset + 1)?;
                let input = self.pop(offset, "type check input");
                let target_type = JavaTypeReport::Object {
                    internal_name: constant_pool.class_name(index)?,
                };
                if opcode == 0xc0 {
                    let output = self.push_value(target_type.clone());
                    self.emit(
                        offset,
                        IrInstructionKindReport::Cast {
                            output,
                            input: input.id,
                            target_type,
                        },
                    );
                } else {
                    let output = self.push_value(JavaTypeReport::Boolean);
                    self.emit(
                        offset,
                        IrInstructionKindReport::InstanceOf {
                            output,
                            input: input.id,
                            target_type,
                        },
                    );
                }
                Ok(offset + 3)
            }
            0xc2 | 0xc3 => {
                let object = self.pop(offset, "monitor object");
                let operation = if opcode == 0xc2 {
                    MonitorOperationReport::Enter
                } else {
                    MonitorOperationReport::Exit
                };
                self.emit(
                    offset,
                    IrInstructionKindReport::Monitor {
                        operation,
                        object: object.id,
                    },
                );
                Ok(offset + 1)
            }
            0xc4 => self.lower_wide(code, offset),
            0xc5 => {
                let index = read_u16_at(code, offset + 1)?;
                let dimensions_count = read_u8_at(code, offset + 3)? as usize;
                let mut dimensions = Vec::with_capacity(dimensions_count);
                for _ in 0..dimensions_count {
                    dimensions.push(self.pop(offset, "multianewarray dimension").id);
                }
                dimensions.reverse();
                let class_name = constant_pool.class_name(index)?;
                let component_type = parse_field_descriptor(&class_name).unwrap_or({
                    JavaTypeReport::Object {
                        internal_name: class_name,
                    }
                });
                let output = self.push_value(JavaTypeReport::Array {
                    dimensions: dimensions_count as u8,
                    component: Box::new(component_type.clone()),
                });
                self.emit(
                    offset,
                    IrInstructionKindReport::NewArray {
                        output,
                        component_type,
                        dimensions,
                    },
                );
                Ok(offset + 4)
            }
            0xc8 => {
                let relative = read_i32_at(code, offset + 1)?;
                self.emit(
                    offset,
                    IrInstructionKindReport::Branch {
                        opcode: opcode_name,
                        condition_values: Vec::new(),
                        target: branch_target(offset, relative)?,
                        fallthrough: None,
                    },
                );
                Ok(offset + 5)
            }
            0xc9 => {
                Ok(self.unsupported_variable_length(offset, opcode, 5, "legacy wide subroutine"))
            }
            _ => self.unsupported_fixed(code, offset, opcode, "opcode not lowered to typed IR"),
        }
    }

    fn seed_exception_handler_stack(&mut self, offset: u32) {
        if !self.stack.is_empty() {
            return;
        }
        if let Some(ty) = self.exception_handler_types.get(&offset).cloned() {
            self.push_value(ty);
        }
    }

    fn push_int_constant(&mut self, offset: usize, value: i32) {
        let output = self.push_value(JavaTypeReport::Int);
        self.emit(
            offset,
            IrInstructionKindReport::Constant {
                output,
                value: ConstantValueReport::Int { value },
            },
        );
    }

    fn push_ldc(
        &mut self,
        offset: usize,
        index: u16,
        constant_pool: &ConstantPoolData,
        category_two: bool,
    ) {
        if let Ok(Some(value)) = constant_pool.string_constant(index) {
            let output = self.push_value(JavaTypeReport::Object {
                internal_name: "java/lang/String".to_owned(),
            });
            self.emit(
                offset,
                IrInstructionKindReport::Constant {
                    output,
                    value: ConstantValueReport::String { value },
                },
            );
            return;
        }

        if let Ok(class) = constant_pool.class_name(index) {
            let output = self.push_value(JavaTypeReport::Object {
                internal_name: "java/lang/Class".to_owned(),
            });
            self.emit(
                offset,
                IrInstructionKindReport::Constant {
                    output,
                    value: ConstantValueReport::Class { descriptor: class },
                },
            );
            return;
        }

        let ty = if category_two {
            JavaTypeReport::Long
        } else {
            JavaTypeReport::Unknown {
                reason: "ldc_constant_not_resolved".to_owned(),
            }
        };
        let output = self.push_value(ty);
        self.emit(
            offset,
            IrInstructionKindReport::Constant {
                output,
                value: ConstantValueReport::Unknown {
                    description: format!("constant-pool entry #{index}"),
                },
            },
        );
    }

    fn load_local(&mut self, offset: usize, local: u16, fallback: JavaTypeReport) {
        let ty = self.local_type(local).unwrap_or(fallback);
        let output = self.push_value(ty);
        self.emit(offset, IrInstructionKindReport::LoadLocal { output, local });
    }

    fn store_local(&mut self, offset: usize, local: u16) {
        let value = self.pop(offset, "store value");
        self.infer_local(local, value.ty.clone());
        self.emit(
            offset,
            IrInstructionKindReport::StoreLocal {
                local,
                value: value.id,
            },
        );
    }

    fn lower_invoke(
        &mut self,
        code: &[u8],
        offset: usize,
        opcode: u8,
        constant_pool: &ConstantPoolData,
    ) -> Result<usize, String> {
        let index = read_u16_at(code, offset + 1)?;
        if opcode == 0xb9 || opcode == 0xba {
            read_u8_at(code, offset + 3)?;
            read_u8_at(code, offset + 4)?;
        }
        let target = self.member_reference(offset, index, constant_pool)?;
        let descriptor = parse_method_descriptor(&target.descriptor).unwrap_or_else(|error| {
            self.errors.push(ir_error(
                Some(offset as u32),
                IrErrorCode::DescriptorParseFailed,
                format!(
                    "cannot parse invoke descriptor {}.{}{}: {error}",
                    target.owner, target.name, target.descriptor
                ),
            ));
            ferrum_model::MethodDescriptorReport {
                parameters: Vec::new(),
                return_type: JavaTypeReport::Unknown {
                    reason: "invalid_invoke_descriptor".to_owned(),
                },
            }
        });

        let mut arguments = Vec::with_capacity(descriptor.parameters.len());
        for _ in descriptor.parameters.iter().rev() {
            arguments.push(self.pop(offset, "invoke argument").id);
        }
        arguments.reverse();
        let receiver = if matches!(opcode, 0xb6 | 0xb7 | 0xb9) {
            Some(self.pop(offset, "invoke receiver").id)
        } else {
            None
        };
        let output = if matches!(descriptor.return_type, JavaTypeReport::Void) {
            None
        } else {
            Some(self.push_value(descriptor.return_type))
        };
        self.emit(
            offset,
            IrInstructionKindReport::Invoke {
                output,
                invocation: invocation_kind(opcode),
                target,
                receiver,
                arguments,
            },
        );
        Ok(if opcode == 0xb9 || opcode == 0xba {
            offset + 5
        } else {
            offset + 3
        })
    }

    fn lower_table_switch(&mut self, code: &[u8], offset: usize) -> Result<usize, String> {
        let input = self.pop(offset, "tableswitch input");
        let mut cursor = aligned_switch_cursor(offset);
        let default_relative = read_i32_at(code, cursor)?;
        cursor += 4;
        let low = read_i32_at(code, cursor)?;
        cursor += 4;
        let high = read_i32_at(code, cursor)?;
        cursor += 4;
        if high < low {
            return Err(format!(
                "tableswitch at {offset} has high value {high} below low value {low}"
            ));
        }
        let case_count = high
            .checked_sub(low)
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| format!("tableswitch at {offset} has too many cases"))?
            as usize;
        let mut cases = Vec::with_capacity(case_count);
        for case_index in 0..case_count {
            let relative = read_i32_at(code, cursor)?;
            cursor += 4;
            cases.push(SwitchCaseReport {
                value: low + case_index as i32,
                target: branch_target(offset, relative)?,
            });
        }
        self.emit(
            offset,
            IrInstructionKindReport::Switch {
                input: input.id,
                default_target: branch_target(offset, default_relative)?,
                cases,
            },
        );
        Ok(cursor)
    }

    fn lower_lookup_switch(&mut self, code: &[u8], offset: usize) -> Result<usize, String> {
        let input = self.pop(offset, "lookupswitch input");
        let mut cursor = aligned_switch_cursor(offset);
        let default_relative = read_i32_at(code, cursor)?;
        cursor += 4;
        let pair_count = read_i32_at(code, cursor)?;
        cursor += 4;
        if pair_count < 0 {
            return Err(format!("lookupswitch at {offset} has negative pair count"));
        }
        let mut cases = Vec::with_capacity(pair_count as usize);
        for _ in 0..pair_count {
            let value = read_i32_at(code, cursor)?;
            cursor += 4;
            let relative = read_i32_at(code, cursor)?;
            cursor += 4;
            cases.push(SwitchCaseReport {
                value,
                target: branch_target(offset, relative)?,
            });
        }
        self.emit(
            offset,
            IrInstructionKindReport::Switch {
                input: input.id,
                default_target: branch_target(offset, default_relative)?,
                cases,
            },
        );
        Ok(cursor)
    }

    fn lower_wide(&mut self, code: &[u8], offset: usize) -> Result<usize, String> {
        let widened_opcode = read_u8_at(code, offset + 1)?;
        match widened_opcode {
            0x15..=0x19 => {
                let local = read_u16_at(code, offset + 2)?;
                self.load_local(offset, local, load_fallback_type(widened_opcode));
                Ok(offset + 4)
            }
            0x36..=0x3a => {
                let local = read_u16_at(code, offset + 2)?;
                self.store_local(offset, local);
                Ok(offset + 4)
            }
            0x84 => {
                let local = read_u16_at(code, offset + 2)?;
                let amount = read_i16_at(code, offset + 4)?;
                self.emit(
                    offset,
                    IrInstructionKindReport::LocalIncrement { local, amount },
                );
                self.infer_local(local, JavaTypeReport::Int);
                Ok(offset + 6)
            }
            _ => {
                ensure_available(code, offset, 4)?;
                self.errors.push(ir_error(
                    Some(offset as u32),
                    IrErrorCode::UnsupportedOpcode,
                    format!(
                        "wide {} is not lowered to typed IR",
                        opcode_name(widened_opcode)
                    ),
                ));
                self.emit(
                    offset,
                    IrInstructionKindReport::Unsupported {
                        opcode: "wide".to_owned(),
                        message: format!(
                            "wide {} is not lowered to typed IR",
                            opcode_name(widened_opcode)
                        ),
                    },
                );
                Ok(offset + 4)
            }
        }
    }

    fn branch_condition_values(&mut self, offset: usize, opcode: u8) -> Vec<u32> {
        if matches!(opcode, 0x9f..=0xa6) {
            let right = self.pop(offset, "branch right operand");
            let left = self.pop(offset, "branch left operand");
            vec![left.id, right.id]
        } else {
            vec![self.pop(offset, "branch condition").id]
        }
    }

    fn unsupported_fixed(
        &mut self,
        code: &[u8],
        offset: usize,
        opcode: u8,
        message: &str,
    ) -> Result<usize, String> {
        let width = fixed_operand_width(opcode)
            .ok_or_else(|| format!("unsupported opcode 0x{opcode:02x} at {offset}"))?;
        ensure_available(code, offset, 1 + width)?;
        let opcode_name = opcode_name(opcode).to_owned();
        self.errors.push(ir_error(
            Some(offset as u32),
            IrErrorCode::UnsupportedOpcode,
            format!("{opcode_name}: {message}"),
        ));
        self.emit(
            offset,
            IrInstructionKindReport::Unsupported {
                opcode: opcode_name,
                message: message.to_owned(),
            },
        );
        Ok(offset + 1 + width)
    }

    fn unsupported_variable_length(
        &mut self,
        offset: usize,
        opcode: u8,
        length: usize,
        message: &str,
    ) -> usize {
        let opcode_name = opcode_name(opcode).to_owned();
        self.errors.push(ir_error(
            Some(offset as u32),
            IrErrorCode::UnsupportedOpcode,
            format!("{opcode_name}: {message}"),
        ));
        self.emit(
            offset,
            IrInstructionKindReport::Unsupported {
                opcode: opcode_name,
                message: message.to_owned(),
            },
        );
        offset + length
    }

    fn member_reference(
        &mut self,
        offset: usize,
        index: u16,
        constant_pool: &ConstantPoolData,
    ) -> Result<MemberReference, String> {
        match constant_pool.member_reference(index)? {
            Some(reference) => Ok(reference),
            None => {
                self.errors.push(ir_error(
                    Some(offset as u32),
                    IrErrorCode::DescriptorParseFailed,
                    format!("constant-pool entry #{index} is not a member reference"),
                ));
                Ok(MemberReference {
                    owner: "<invalid>".to_owned(),
                    name: format!("constant_{index}"),
                    descriptor: "()V".to_owned(),
                })
            }
        }
    }

    fn emit(&mut self, offset: usize, kind: IrInstructionKindReport) {
        let id = self.next_instruction_id;
        self.next_instruction_id = self.next_instruction_id.saturating_add(1);
        self.instructions.push(IrInstructionReport {
            id,
            source: IrSourceReport {
                class: self.owner.to_owned(),
                method: self.method.to_owned(),
                descriptor: self.descriptor.to_owned(),
                bytecode_offset: offset as u32,
                line_number: line_number_for_offset(self.line_number_table, offset as u32),
            },
            kind,
        });
    }

    fn push_value(&mut self, ty: JavaTypeReport) -> IrValueReport {
        let id = self.next_value_id;
        self.next_value_id = self.next_value_id.saturating_add(1);
        self.stack.push(StackValue { id, ty: ty.clone() });
        IrValueReport { id, ty }
    }

    fn new_value(&mut self, ty: JavaTypeReport) -> IrValueReport {
        let id = self.next_value_id;
        self.next_value_id = self.next_value_id.saturating_add(1);
        IrValueReport { id, ty }
    }

    fn pop(&mut self, offset: usize, purpose: &str) -> StackValue {
        match self.stack.pop() {
            Some(value) => value,
            None => {
                self.errors.push(ir_error(
                    Some(offset as u32),
                    IrErrorCode::StackUnderflow,
                    format!("stack underflow while reading {purpose}"),
                ));
                let value = self.new_value(JavaTypeReport::Unknown {
                    reason: "stack_underflow".to_owned(),
                });
                StackValue {
                    id: value.id,
                    ty: value.ty,
                }
            }
        }
    }

    fn peek_or_unknown(&mut self, offset: usize, purpose: &str) -> StackValue {
        match self.stack.last().cloned() {
            Some(value) => value,
            None => {
                self.errors.push(ir_error(
                    Some(offset as u32),
                    IrErrorCode::StackUnderflow,
                    format!("stack underflow while reading {purpose}"),
                ));
                let value = self.new_value(JavaTypeReport::Unknown {
                    reason: "stack_underflow".to_owned(),
                });
                StackValue {
                    id: value.id,
                    ty: value.ty,
                }
            }
        }
    }

    fn local_type(&self, slot: u16) -> Option<JavaTypeReport> {
        self.locals.get(&slot).map(|local| local.ty.clone())
    }

    fn infer_local(&mut self, slot: u16, ty: JavaTypeReport) {
        self.locals
            .entry(slot)
            .and_modify(|local| {
                if matches!(local.ty, JavaTypeReport::Unknown { .. }) {
                    local.ty = ty.clone();
                    local.slot_width = local.ty.slot_width();
                    local.source = IrLocalSource::StoreInference;
                }
            })
            .or_insert_with(|| IrLocalReport {
                slot,
                name: None,
                slot_width: ty.slot_width(),
                ty,
                source: IrLocalSource::StoreInference,
                start_pc: None,
                end_pc: None,
            });
    }

    fn add_cfg_merges(&mut self, cfg: &ControlFlowGraphReport) -> Vec<IrMergeValueReport> {
        let mut predecessors: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for block in &cfg.blocks {
            for successor in &block.successors {
                predecessors.entry(*successor).or_default().push(block.id);
            }
        }

        let mut merges = Vec::new();
        for block in &cfg.blocks {
            let Some(inputs) = predecessors.get(&block.id) else {
                continue;
            };
            if inputs.len() < 2 {
                continue;
            }
            let output = self.new_value(JavaTypeReport::Unknown {
                reason: "control_flow_merge".to_owned(),
            });
            self.errors.push(ir_error(
                Some(block.bytecode_start),
                IrErrorCode::ConservativeMerge,
                format!(
                    "block {} has {} predecessors; inserted conservative phi placeholder",
                    block.id,
                    inputs.len()
                ),
            ));
            self.emit(
                block.bytecode_start as usize,
                IrInstructionKindReport::Phi {
                    output: output.clone(),
                    block: block.id,
                    inputs: Vec::new(),
                },
            );
            merges.push(IrMergeValueReport {
                block: block.id,
                bytecode_offset: block.bytecode_start,
                output,
                inputs: Vec::new(),
            });
        }
        merges
    }
}

fn ir_error(offset: Option<u32>, code: IrErrorCode, message: String) -> IrErrorReport {
    IrErrorReport {
        offset,
        code,
        message,
    }
}

fn line_number_for_offset(table: &[LineNumberReport], offset: u32) -> Option<u16> {
    table
        .iter()
        .filter(|line| u32::from(line.start_pc) <= offset)
        .max_by_key(|line| line.start_pc)
        .map(|line| line.line_number)
}

fn compact_load(opcode: u8) -> (u16, JavaTypeReport) {
    match opcode {
        0x1a..=0x1d => ((opcode - 0x1a) as u16, JavaTypeReport::Int),
        0x1e..=0x21 => ((opcode - 0x1e) as u16, JavaTypeReport::Long),
        0x22..=0x25 => ((opcode - 0x22) as u16, JavaTypeReport::Float),
        0x26..=0x29 => ((opcode - 0x26) as u16, JavaTypeReport::Double),
        0x2a..=0x2d => (
            (opcode - 0x2a) as u16,
            JavaTypeReport::Unknown {
                reason: "reference_local".to_owned(),
            },
        ),
        _ => unreachable!("compact load opcode checked by caller"),
    }
}

fn compact_store(opcode: u8) -> u16 {
    match opcode {
        0x3b..=0x3e => (opcode - 0x3b) as u16,
        0x3f..=0x42 => (opcode - 0x3f) as u16,
        0x43..=0x46 => (opcode - 0x43) as u16,
        0x47..=0x4a => (opcode - 0x47) as u16,
        0x4b..=0x4e => (opcode - 0x4b) as u16,
        _ => unreachable!("compact store opcode checked by caller"),
    }
}

fn load_fallback_type(opcode: u8) -> JavaTypeReport {
    match opcode {
        0x15 => JavaTypeReport::Int,
        0x16 => JavaTypeReport::Long,
        0x17 => JavaTypeReport::Float,
        0x18 => JavaTypeReport::Double,
        _ => JavaTypeReport::Unknown {
            reason: "reference_local".to_owned(),
        },
    }
}

fn array_load_type(opcode: u8) -> JavaTypeReport {
    match opcode {
        0x2e => JavaTypeReport::Int,
        0x2f => JavaTypeReport::Long,
        0x30 => JavaTypeReport::Float,
        0x31 => JavaTypeReport::Double,
        0x33 => JavaTypeReport::Byte,
        0x34 => JavaTypeReport::Char,
        0x35 => JavaTypeReport::Short,
        _ => JavaTypeReport::Unknown {
            reason: "array_component".to_owned(),
        },
    }
}

fn binary_output_type(opcode: u8) -> JavaTypeReport {
    match opcode {
        0x61 | 0x65 | 0x69 | 0x6d | 0x71 | 0x79 | 0x7b | 0x7d | 0x7f | 0x81 | 0x83 => {
            JavaTypeReport::Long
        }
        0x62 | 0x66 | 0x6a | 0x6e | 0x72 => JavaTypeReport::Float,
        0x63 | 0x67 | 0x6b | 0x6f | 0x73 => JavaTypeReport::Double,
        0x94..=0x98 => JavaTypeReport::Int,
        _ => JavaTypeReport::Int,
    }
}

fn unary_output_type(opcode: u8) -> JavaTypeReport {
    match opcode {
        0x75 => JavaTypeReport::Long,
        0x76 => JavaTypeReport::Float,
        0x77 => JavaTypeReport::Double,
        _ => JavaTypeReport::Int,
    }
}

fn conversion_output_type(opcode: u8) -> JavaTypeReport {
    match opcode {
        0x85 | 0x8c | 0x8f => JavaTypeReport::Long,
        0x86 | 0x89 | 0x90 => JavaTypeReport::Float,
        0x87 | 0x8a | 0x8d => JavaTypeReport::Double,
        _ => JavaTypeReport::Int,
    }
}

fn primitive_array_type(atype: u8) -> JavaTypeReport {
    match atype {
        4 => JavaTypeReport::Boolean,
        5 => JavaTypeReport::Char,
        6 => JavaTypeReport::Float,
        7 => JavaTypeReport::Double,
        8 => JavaTypeReport::Byte,
        9 => JavaTypeReport::Short,
        10 => JavaTypeReport::Int,
        11 => JavaTypeReport::Long,
        _ => JavaTypeReport::Unknown {
            reason: format!("newarray_atype_{atype}"),
        },
    }
}

fn invocation_kind(opcode: u8) -> InvocationKindReport {
    match opcode {
        0xb8 => InvocationKindReport::Static,
        0xb7 => InvocationKindReport::Special,
        0xb9 => InvocationKindReport::Interface,
        0xba => InvocationKindReport::Dynamic,
        _ => InvocationKindReport::Virtual,
    }
}

fn build_cfg(
    owner: &str,
    method: &str,
    descriptor: &str,
    report: &MethodBytecodeReport,
) -> ControlFlowGraphReport {
    let code_length = report.code_length.unwrap_or_default();
    let instruction_offsets: BTreeSet<u32> = report
        .instructions
        .iter()
        .map(|instruction| instruction.offset)
        .collect();
    let mut leaders = BTreeSet::new();
    let mut errors = Vec::new();

    if let Some(first) = report.instructions.first() {
        leaders.insert(first.offset);
    }

    for branch in &report.branches {
        if let Some(target) =
            valid_instruction_target(branch.target, &instruction_offsets, code_length)
        {
            leaders.insert(target);
        } else {
            errors.push(cfg_error(
                branch.offset,
                CfgErrorCode::InvalidBranchTarget,
                format!(
                    "{} at {} targets invalid bytecode offset {}",
                    branch.opcode, branch.offset, branch.target
                ),
            ));
        }

        if let Some(next_offset) = next_instruction_offset(branch.offset, &instruction_offsets) {
            leaders.insert(next_offset);
        }
    }

    for switch in &report.switches {
        if let Some(default_target) =
            valid_instruction_target(switch.default_target, &instruction_offsets, code_length)
        {
            leaders.insert(default_target);
        } else {
            errors.push(cfg_error(
                switch.offset,
                CfgErrorCode::InvalidSwitchTarget,
                format!(
                    "{} at {} has invalid default target {}",
                    switch.opcode, switch.offset, switch.default_target
                ),
            ));
        }

        for case in &switch.cases {
            if let Some(target) =
                valid_instruction_target(case.target, &instruction_offsets, code_length)
            {
                leaders.insert(target);
            } else {
                errors.push(cfg_error(
                    switch.offset,
                    CfgErrorCode::InvalidSwitchTarget,
                    format!(
                        "{} at {} case {} targets invalid bytecode offset {}",
                        switch.opcode, switch.offset, case.value, case.target
                    ),
                ));
            }
        }

        if let Some(next_offset) = next_instruction_offset(switch.offset, &instruction_offsets) {
            leaders.insert(next_offset);
        }
    }

    for terminal in report
        .returns
        .iter()
        .map(|instruction| instruction.offset)
        .chain(report.throws.iter().map(|instruction| instruction.offset))
    {
        if let Some(next_offset) = next_instruction_offset(terminal, &instruction_offsets) {
            leaders.insert(next_offset);
        }
    }

    for handler in &report.exception_table {
        if let Some(handler_target) =
            valid_instruction_target(handler.handler_pc.into(), &instruction_offsets, code_length)
        {
            leaders.insert(handler_target);
        } else {
            errors.push(cfg_error(
                handler.handler_pc.into(),
                CfgErrorCode::InvalidExceptionHandlerTarget,
                format!(
                    "exception handler targets invalid bytecode offset {}",
                    handler.handler_pc
                ),
            ));
        }
        if instruction_offsets.contains(&u32::from(handler.start_pc)) {
            leaders.insert(handler.start_pc.into());
        }
        if instruction_offsets.contains(&u32::from(handler.end_pc)) {
            leaders.insert(handler.end_pc.into());
        }
    }

    leaders.retain(|offset| instruction_offsets.contains(offset));
    let leader_offsets: Vec<_> = leaders.into_iter().collect();
    let block_by_start: BTreeMap<u32, u32> = leader_offsets
        .iter()
        .enumerate()
        .map(|(index, offset)| (*offset, index as u32))
        .collect();
    let branch_by_offset: BTreeMap<_, _> = report
        .branches
        .iter()
        .map(|branch| (branch.offset, branch))
        .collect();
    let switch_by_offset: BTreeMap<_, _> = report
        .switches
        .iter()
        .map(|switch| (switch.offset, switch))
        .collect();
    let return_offsets: BTreeSet<_> = report
        .returns
        .iter()
        .map(|instruction| instruction.offset)
        .collect();
    let throw_offsets: BTreeSet<_> = report
        .throws
        .iter()
        .map(|instruction| instruction.offset)
        .collect();
    let lookups = CfgLookups {
        block_by_start: &block_by_start,
        branch_by_offset: &branch_by_offset,
        switch_by_offset: &switch_by_offset,
        return_offsets: &return_offsets,
        throw_offsets: &throw_offsets,
    };

    let mut blocks = Vec::with_capacity(leader_offsets.len());
    for (index, start) in leader_offsets.iter().copied().enumerate() {
        let end = leader_offsets
            .get(index + 1)
            .copied()
            .unwrap_or(code_length);
        let instructions: Vec<_> = report
            .instructions
            .iter()
            .filter(|instruction| instruction.offset >= start && instruction.offset < end)
            .cloned()
            .collect();
        let terminator = block_terminator(start, end, &instructions, &lookups);
        let successors = terminator_successors(&terminator);

        blocks.push(BasicBlockReport {
            id: index as u32,
            bytecode_start: start,
            bytecode_end: end,
            instructions,
            terminator,
            successors,
        });
    }

    let exception_handlers =
        cfg_exception_handlers(report, &block_by_start, &mut blocks, &mut errors);
    for block in &mut blocks {
        block.successors.sort_unstable();
        block.successors.dedup();
    }

    let dot = cfg_dot(owner, method, descriptor, &blocks);

    ControlFlowGraphReport {
        owner: owner.to_owned(),
        method: method.to_owned(),
        descriptor: descriptor.to_owned(),
        blocks,
        exception_handlers,
        errors,
        dot,
    }
}

struct CfgLookups<'a> {
    block_by_start: &'a BTreeMap<u32, u32>,
    branch_by_offset: &'a BTreeMap<u32, &'a BranchInstructionReport>,
    switch_by_offset: &'a BTreeMap<u32, &'a SwitchInstructionReport>,
    return_offsets: &'a BTreeSet<u32>,
    throw_offsets: &'a BTreeSet<u32>,
}

fn block_terminator(
    start: u32,
    end: u32,
    instructions: &[InstructionReport],
    lookups: &CfgLookups<'_>,
) -> TerminatorReport {
    let Some(last) = instructions.last() else {
        return TerminatorReport::Unreachable;
    };

    if let Some(switch) = lookups.switch_by_offset.get(&last.offset) {
        let Some(default_block) = target_block(switch.default_target, lookups.block_by_start)
        else {
            return TerminatorReport::Unreachable;
        };
        let mut cases = Vec::with_capacity(switch.cases.len());
        for case in &switch.cases {
            let Some(block) = target_block(case.target, lookups.block_by_start) else {
                return TerminatorReport::Unreachable;
            };
            cases.push(SwitchBlockCaseReport {
                value: case.value,
                block,
            });
        }
        return TerminatorReport::Switch {
            cases,
            default_block,
        };
    }

    if let Some(branch) = lookups.branch_by_offset.get(&last.offset) {
        let Some(branch_block) = target_block(branch.target, lookups.block_by_start) else {
            return TerminatorReport::Unreachable;
        };
        if is_conditional_branch(&branch.opcode) {
            let Some(fallthrough_block) = lookups.block_by_start.get(&end).copied() else {
                return TerminatorReport::Unreachable;
            };
            return TerminatorReport::Branch {
                then_block: branch_block,
                else_block: fallthrough_block,
            };
        }
        return TerminatorReport::Goto {
            block: branch_block,
        };
    }

    if lookups.return_offsets.contains(&last.offset) {
        return TerminatorReport::Return;
    }
    if lookups.throw_offsets.contains(&last.offset) {
        return TerminatorReport::Throw;
    }
    if let Some(block) = lookups.block_by_start.get(&end).copied() {
        return TerminatorReport::Fallthrough { block };
    }

    if start == end {
        TerminatorReport::Unreachable
    } else {
        TerminatorReport::Return
    }
}

fn cfg_exception_handlers(
    report: &MethodBytecodeReport,
    block_by_start: &BTreeMap<u32, u32>,
    blocks: &mut [BasicBlockReport],
    errors: &mut Vec<CfgErrorReport>,
) -> Vec<CfgExceptionHandlerReport> {
    let mut handlers = Vec::with_capacity(report.exception_table.len());
    for handler in &report.exception_table {
        let Some(handler_block) = block_by_start.get(&u32::from(handler.handler_pc)).copied()
        else {
            errors.push(cfg_error(
                handler.handler_pc.into(),
                CfgErrorCode::InvalidExceptionHandlerTarget,
                format!(
                    "exception handler target {} did not map to a basic block",
                    handler.handler_pc
                ),
            ));
            continue;
        };

        let mut covered_blocks = Vec::new();
        for block in blocks.iter_mut() {
            if ranges_overlap(
                block.bytecode_start,
                block.bytecode_end,
                handler.start_pc.into(),
                handler.end_pc.into(),
            ) {
                covered_blocks.push(block.id);
                block.successors.push(handler_block);
            }
        }

        handlers.push(CfgExceptionHandlerReport {
            start_pc: handler.start_pc,
            end_pc: handler.end_pc,
            handler_pc: handler.handler_pc,
            handler_block,
            catch_type: handler.catch_type.clone(),
            covered_blocks,
        });
    }
    handlers
}

fn cfg_dot(owner: &str, method: &str, descriptor: &str, blocks: &[BasicBlockReport]) -> String {
    let mut dot = String::new();
    dot.push_str("digraph ferrum_cfg {\n");
    dot.push_str("  rankdir=TB;\n");
    dot.push_str("  node [shape=box,fontname=\"Consolas\"];\n");
    dot.push_str(&format!(
        "  label=\"{}\";\n",
        dot_escape(&format!("{owner}.{method}{descriptor}"))
    ));

    for block in blocks {
        let mut label = format!(
            "B{} @{}..{}\\n{}",
            block.id,
            block.bytecode_start,
            block.bytecode_end,
            terminator_name(&block.terminator)
        );
        for instruction in &block.instructions {
            label.push_str(&format!(
                "\\n{}: {}",
                instruction.offset, instruction.opcode
            ));
        }
        dot.push_str(&format!(
            "  b{} [label=\"{}\"];\n",
            block.id,
            dot_escape(&label)
        ));
    }

    for block in blocks {
        for successor in &block.successors {
            dot.push_str(&format!("  b{} -> b{};\n", block.id, successor));
        }
    }

    dot.push_str("}\n");
    dot
}

fn terminator_successors(terminator: &TerminatorReport) -> Vec<u32> {
    let mut successors = Vec::new();
    match terminator {
        TerminatorReport::Fallthrough { block } | TerminatorReport::Goto { block } => {
            successors.push(*block);
        }
        TerminatorReport::Branch {
            then_block,
            else_block,
        } => {
            successors.push(*then_block);
            successors.push(*else_block);
        }
        TerminatorReport::Switch {
            cases,
            default_block,
        } => {
            successors.push(*default_block);
            successors.extend(cases.iter().map(|case| case.block));
        }
        TerminatorReport::Return | TerminatorReport::Throw | TerminatorReport::Unreachable => {}
    }
    successors.sort_unstable();
    successors.dedup();
    successors
}

fn valid_instruction_target(
    target: i32,
    instruction_offsets: &BTreeSet<u32>,
    code_length: u32,
) -> Option<u32> {
    let target = u32::try_from(target).ok()?;
    if target < code_length && instruction_offsets.contains(&target) {
        Some(target)
    } else {
        None
    }
}

fn target_block(target: i32, block_by_start: &BTreeMap<u32, u32>) -> Option<u32> {
    u32::try_from(target)
        .ok()
        .and_then(|target| block_by_start.get(&target).copied())
}

fn next_instruction_offset(offset: u32, instruction_offsets: &BTreeSet<u32>) -> Option<u32> {
    instruction_offsets
        .iter()
        .copied()
        .find(|candidate| *candidate > offset)
}

fn ranges_overlap(left_start: u32, left_end: u32, right_start: u32, right_end: u32) -> bool {
    left_start < right_end && right_start < left_end
}

fn is_conditional_branch(opcode: &str) -> bool {
    opcode.starts_with("if")
}

fn cfg_error(offset: u32, code: CfgErrorCode, message: String) -> CfgErrorReport {
    CfgErrorReport {
        offset,
        code,
        message,
    }
}

fn terminator_name(terminator: &TerminatorReport) -> &'static str {
    match terminator {
        TerminatorReport::Fallthrough { block: _ } => "fallthrough",
        TerminatorReport::Branch {
            then_block: _,
            else_block: _,
        } => "branch",
        TerminatorReport::Goto { block: _ } => "goto",
        TerminatorReport::Switch {
            cases: _,
            default_block: _,
        } => "switch",
        TerminatorReport::Return => "return",
        TerminatorReport::Throw => "throw",
        TerminatorReport::Unreachable => "unreachable",
    }
}

fn dot_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

struct AnalysisCollector {
    instructions: Vec<InstructionReport>,
    opcode_histogram: BTreeMap<String, u64>,
    branches: Vec<BranchInstructionReport>,
    switches: Vec<SwitchInstructionReport>,
    returns: Vec<ReturnInstructionReport>,
    throws: Vec<ThrowInstructionReport>,
    referenced_methods: BTreeSet<MemberReference>,
    referenced_fields: BTreeSet<MemberReference>,
    referenced_types: BTreeSet<String>,
    string_constants: BTreeSet<String>,
    features: BTreeSet<BytecodeFeature>,
    reason_codes: BTreeSet<ClassificationReason>,
}

fn decode_code(
    code: &[u8],
    constant_pool: &ConstantPoolData,
    bootstrap_methods: &[BootstrapMethod],
    collector: &mut AnalysisCollector,
) -> Result<(), String> {
    let mut offset = 0usize;
    while offset < code.len() {
        let opcode = code[offset];
        let opcode_name = opcode_name(opcode);
        collector.instructions.push(InstructionReport {
            offset: offset as u32,
            opcode: opcode_name.to_owned(),
        });
        *collector
            .opcode_histogram
            .entry(opcode_name.to_owned())
            .or_insert(0) += 1;

        match opcode {
            0x12 => {
                let index = read_u8_at(code, offset + 1)? as u16;
                handle_ldc(index, constant_pool, collector)?;
                offset += 2;
            }
            0x13 | 0x14 => {
                let index = read_u16_at(code, offset + 1)?;
                handle_ldc(index, constant_pool, collector)?;
                offset += 3;
            }
            0x99..=0xa8 | 0xc6 | 0xc7 => {
                let relative = read_i16_at(code, offset + 1)? as i32;
                add_branch(offset, opcode_name, relative, collector)?;
                if opcode == 0xa8 {
                    add_legacy_subroutine(collector);
                }
                offset += 3;
            }
            0xa9 => {
                read_u8_at(code, offset + 1)?;
                add_legacy_subroutine(collector);
                offset += 2;
            }
            0xaa => {
                offset = decode_table_switch(code, offset, collector)?;
            }
            0xab => {
                offset = decode_lookup_switch(code, offset, collector)?;
            }
            0xac..=0xb1 => {
                collector.returns.push(ReturnInstructionReport {
                    offset: offset as u32,
                    opcode: opcode_name.to_owned(),
                });
                offset += 1;
            }
            0xb2..=0xb5 => {
                let index = read_u16_at(code, offset + 1)?;
                if let Some(reference) = constant_pool.member_reference(index)? {
                    collect_reference_types(&reference, &mut collector.referenced_types);
                    detect_reference_features(&reference, collector);
                    collector.referenced_fields.insert(reference);
                }
                offset += 3;
            }
            0xb6..=0xb8 => {
                let index = read_u16_at(code, offset + 1)?;
                if let Some(reference) = constant_pool.member_reference(index)? {
                    collect_reference_types(&reference, &mut collector.referenced_types);
                    detect_reference_features(&reference, collector);
                    collector.referenced_methods.insert(reference);
                }
                if opcode == 0xb6 {
                    collector.features.insert(BytecodeFeature::VirtualDispatch);
                    collector
                        .reason_codes
                        .insert(ClassificationReason::UsesVirtualDispatch);
                }
                offset += 3;
            }
            0xb9 => {
                let index = read_u16_at(code, offset + 1)?;
                read_u8_at(code, offset + 3)?;
                read_u8_at(code, offset + 4)?;
                if let Some(reference) = constant_pool.member_reference(index)? {
                    collect_reference_types(&reference, &mut collector.referenced_types);
                    detect_reference_features(&reference, collector);
                    collector.referenced_methods.insert(reference);
                }
                collector.features.insert(BytecodeFeature::VirtualDispatch);
                collector
                    .reason_codes
                    .insert(ClassificationReason::UsesVirtualDispatch);
                offset += 5;
            }
            0xba => {
                let index = read_u16_at(code, offset + 1)?;
                read_u8_at(code, offset + 3)?;
                read_u8_at(code, offset + 4)?;
                if let Some(reference) = constant_pool.member_reference(index)? {
                    collect_reference_types(&reference, &mut collector.referenced_types);
                    collector.referenced_methods.insert(reference);
                }
                collector.features.insert(BytecodeFeature::InvokeDynamic);
                collector
                    .reason_codes
                    .insert(ClassificationReason::UsesInvokeDynamic);
                handle_bootstrap_method(index, constant_pool, bootstrap_methods, collector)?;
                offset += 5;
            }
            0xbb => {
                let index = read_u16_at(code, offset + 1)?;
                collector
                    .referenced_types
                    .insert(constant_pool.class_name(index)?);
                collector.features.insert(BytecodeFeature::ObjectAllocation);
                collector
                    .reason_codes
                    .insert(ClassificationReason::AllocatesObjects);
                offset += 3;
            }
            0xbc => {
                read_u8_at(code, offset + 1)?;
                add_array_operation(collector);
                offset += 2;
            }
            0xbd | 0xc0 | 0xc1 => {
                let index = read_u16_at(code, offset + 1)?;
                collector
                    .referenced_types
                    .insert(constant_pool.class_name(index)?);
                if opcode == 0xbd {
                    add_array_operation(collector);
                }
                offset += 3;
            }
            0xbf => {
                collector.throws.push(ThrowInstructionReport {
                    offset: offset as u32,
                });
                collector.features.insert(BytecodeFeature::ThrowInstruction);
                collector
                    .reason_codes
                    .insert(ClassificationReason::ThrowsExceptions);
                offset += 1;
            }
            0xc2 => {
                collector.features.insert(BytecodeFeature::MonitorEnter);
                collector
                    .reason_codes
                    .insert(ClassificationReason::UsesMonitor);
                offset += 1;
            }
            0xc3 => {
                collector.features.insert(BytecodeFeature::MonitorExit);
                collector
                    .reason_codes
                    .insert(ClassificationReason::UsesMonitor);
                offset += 1;
            }
            0xc4 => {
                offset = decode_wide(code, offset, collector)?;
            }
            0xc5 => {
                let index = read_u16_at(code, offset + 1)?;
                read_u8_at(code, offset + 3)?;
                collector
                    .referenced_types
                    .insert(constant_pool.class_name(index)?);
                add_array_operation(collector);
                offset += 4;
            }
            0xc8 | 0xc9 => {
                let relative = read_i32_at(code, offset + 1)?;
                add_branch(offset, opcode_name, relative, collector)?;
                if opcode == 0xc9 {
                    add_legacy_subroutine(collector);
                }
                offset += 5;
            }
            _ => {
                if is_array_opcode(opcode) {
                    add_array_operation(collector);
                }
                let operand_width = fixed_operand_width(opcode)
                    .ok_or_else(|| format!("unsupported opcode 0x{opcode:02x} at {offset}"))?;
                ensure_available(code, offset, 1 + operand_width)?;
                offset += 1 + operand_width;
            }
        }
    }

    Ok(())
}

fn handle_ldc(
    index: u16,
    constant_pool: &ConstantPoolData,
    collector: &mut AnalysisCollector,
) -> Result<(), String> {
    if let Some(value) = constant_pool.string_constant(index)? {
        detect_string_features(&value, collector);
        collector.string_constants.insert(value);
    }
    if let Ok(class_name) = constant_pool.class_name(index) {
        collector.referenced_types.insert(class_name);
    }
    if let Some(descriptor) = constant_pool.method_type_descriptor(index)? {
        collect_descriptor_types(&descriptor, &mut collector.referenced_types);
    }
    if let Some(reference) = constant_pool.member_reference(index)? {
        collect_reference_types(&reference, &mut collector.referenced_types);
        detect_reference_features(&reference, collector);
        collector.referenced_methods.insert(reference);
    }
    Ok(())
}

fn handle_bootstrap_method(
    invoke_dynamic_index: u16,
    constant_pool: &ConstantPoolData,
    bootstrap_methods: &[BootstrapMethod],
    collector: &mut AnalysisCollector,
) -> Result<(), String> {
    let Some(bootstrap_index) = constant_pool.bootstrap_method_attr_index(invoke_dynamic_index)?
    else {
        return Ok(());
    };
    let Some(bootstrap_method) = bootstrap_methods.get(bootstrap_index as usize) else {
        return Ok(());
    };

    if let Some(reference) = constant_pool.member_reference(bootstrap_method.method_ref)? {
        collect_reference_types(&reference, &mut collector.referenced_types);
        detect_reference_features(&reference, collector);
        collector.referenced_methods.insert(reference);
    }
    for argument in &bootstrap_method.arguments {
        handle_ldc(*argument, constant_pool, collector)?;
    }
    Ok(())
}

fn decode_table_switch(
    code: &[u8],
    offset: usize,
    collector: &mut AnalysisCollector,
) -> Result<usize, String> {
    let mut cursor = aligned_switch_cursor(offset);
    let default_relative = read_i32_at(code, cursor)?;
    cursor += 4;
    let low = read_i32_at(code, cursor)?;
    cursor += 4;
    let high = read_i32_at(code, cursor)?;
    cursor += 4;
    if high < low {
        return Err(format!(
            "tableswitch at {offset} has high value {high} below low value {low}"
        ));
    }

    let case_count =
        high.checked_sub(low)
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| format!("tableswitch at {offset} has too many cases"))? as usize;
    let available_cases = code.len().saturating_sub(cursor) / 4;
    if case_count > available_cases {
        return Err(format!(
            "tableswitch at {offset} declares {case_count} cases but only {available_cases} fit"
        ));
    }
    let mut cases = Vec::with_capacity(case_count);
    for case_index in 0..case_count {
        let relative = read_i32_at(code, cursor)?;
        cursor += 4;
        cases.push(SwitchCaseReport {
            value: low + case_index as i32,
            target: branch_target(offset, relative)?,
        });
    }

    collector
        .features
        .insert(BytecodeFeature::SwitchInstruction);
    collector
        .reason_codes
        .insert(ClassificationReason::UsesSwitch);
    collector.switches.push(SwitchInstructionReport {
        offset: offset as u32,
        opcode: "tableswitch".to_owned(),
        default_target: branch_target(offset, default_relative)?,
        cases,
    });
    Ok(cursor)
}

fn decode_lookup_switch(
    code: &[u8],
    offset: usize,
    collector: &mut AnalysisCollector,
) -> Result<usize, String> {
    let mut cursor = aligned_switch_cursor(offset);
    let default_relative = read_i32_at(code, cursor)?;
    cursor += 4;
    let pair_count = read_i32_at(code, cursor)?;
    cursor += 4;
    if pair_count < 0 {
        return Err(format!("lookupswitch at {offset} has negative pair count"));
    }

    let pair_count = pair_count as usize;
    let available_pairs = code.len().saturating_sub(cursor) / 8;
    if pair_count > available_pairs {
        return Err(format!(
            "lookupswitch at {offset} declares {pair_count} pairs but only {available_pairs} fit"
        ));
    }

    let mut cases = Vec::with_capacity(pair_count);
    for _ in 0..pair_count {
        let value = read_i32_at(code, cursor)?;
        cursor += 4;
        let relative = read_i32_at(code, cursor)?;
        cursor += 4;
        cases.push(SwitchCaseReport {
            value,
            target: branch_target(offset, relative)?,
        });
    }

    collector
        .features
        .insert(BytecodeFeature::SwitchInstruction);
    collector
        .reason_codes
        .insert(ClassificationReason::UsesSwitch);
    collector.switches.push(SwitchInstructionReport {
        offset: offset as u32,
        opcode: "lookupswitch".to_owned(),
        default_target: branch_target(offset, default_relative)?,
        cases,
    });
    Ok(cursor)
}

fn decode_wide(
    code: &[u8],
    offset: usize,
    collector: &mut AnalysisCollector,
) -> Result<usize, String> {
    let widened_opcode = read_u8_at(code, offset + 1)?;
    if widened_opcode == 0xa9 {
        add_legacy_subroutine(collector);
    }
    if widened_opcode == 0x84 {
        ensure_available(code, offset, 6)?;
        Ok(offset + 6)
    } else {
        ensure_available(code, offset, 4)?;
        Ok(offset + 4)
    }
}

fn add_branch(
    offset: usize,
    opcode: &str,
    relative: i32,
    collector: &mut AnalysisCollector,
) -> Result<(), String> {
    collector.branches.push(BranchInstructionReport {
        offset: offset as u32,
        opcode: opcode.to_owned(),
        target: branch_target(offset, relative)?,
    });
    Ok(())
}

fn branch_target(offset: usize, relative: i32) -> Result<i32, String> {
    let base =
        i32::try_from(offset).map_err(|_| format!("bytecode offset {offset} is too large"))?;
    base.checked_add(relative)
        .ok_or_else(|| format!("branch at {offset} overflows target offset"))
}

fn aligned_switch_cursor(offset: usize) -> usize {
    let mut cursor = offset + 1;
    while cursor % 4 != 0 {
        cursor += 1;
    }
    cursor
}

fn add_legacy_subroutine(collector: &mut AnalysisCollector) {
    collector.features.insert(BytecodeFeature::LegacySubroutine);
    collector
        .reason_codes
        .insert(ClassificationReason::UsesLegacySubroutine);
}

fn add_array_operation(collector: &mut AnalysisCollector) {
    collector.features.insert(BytecodeFeature::ArrayOperation);
    collector
        .reason_codes
        .insert(ClassificationReason::UsesArrays);
}

fn detect_reference_features(reference: &MemberReference, collector: &mut AnalysisCollector) {
    let owner = reference.owner.as_str();
    if owner.starts_with("java/lang/reflect/")
        || (owner == "java/lang/Class" && is_reflection_method(&reference.name))
    {
        collector.features.insert(BytecodeFeature::ReflectionApi);
        collector
            .reason_codes
            .insert(ClassificationReason::UsesReflection);
    }
    if owner == "java/lang/reflect/Proxy" {
        collector.features.insert(BytecodeFeature::DynamicProxy);
        collector
            .reason_codes
            .insert(ClassificationReason::UsesDynamicProxy);
    }
    if owner == "java/lang/ClassLoader"
        || owner.ends_with("/ClassLoader")
        || is_class_loader_method(&reference.name)
    {
        collector
            .features
            .insert(BytecodeFeature::CustomClassLoader);
        collector
            .reason_codes
            .insert(ClassificationReason::UsesCustomClassLoading);
    }
    if owner == "sun/misc/Unsafe" || owner == "jdk/internal/misc/Unsafe" {
        collector.features.insert(BytecodeFeature::UnsafeApi);
        collector
            .reason_codes
            .insert(ClassificationReason::UsesUnsafe);
    }
    if (owner == "java/lang/System" || owner == "java/lang/Runtime")
        && (reference.name == "load" || reference.name == "loadLibrary")
    {
        collector
            .features
            .insert(BytecodeFeature::NativeLibraryLoading);
        collector
            .reason_codes
            .insert(ClassificationReason::LoadsNativeLibrary);
    }
    if owner.starts_with("java/lang/invoke/") {
        collector.features.insert(BytecodeFeature::JavaLangInvoke);
        collector
            .reason_codes
            .insert(ClassificationReason::UsesJavaLangInvoke);
    }
    if owner == "java/lang/invoke/LambdaMetafactory" {
        collector
            .features
            .insert(BytecodeFeature::LambdaMetafactory);
        collector
            .reason_codes
            .insert(ClassificationReason::UsesLambdaMetafactory);
    }
    if is_runtime_bytecode_generation_owner(owner) {
        collector
            .features
            .insert(BytecodeFeature::RuntimeBytecodeGeneration);
        collector
            .reason_codes
            .insert(ClassificationReason::UsesRuntimeBytecodeGeneration);
    }
}

fn detect_string_features(value: &str, collector: &mut AnalysisCollector) {
    let internalized = value.replace('.', "/");
    if is_runtime_bytecode_generation_owner(&internalized) {
        collector
            .features
            .insert(BytecodeFeature::RuntimeBytecodeGeneration);
        collector
            .reason_codes
            .insert(ClassificationReason::UsesRuntimeBytecodeGeneration);
    }
}

fn is_reflection_method(name: &str) -> bool {
    matches!(
        name,
        "forName"
            | "getClass"
            | "getClasses"
            | "getDeclaredClasses"
            | "getConstructor"
            | "getConstructors"
            | "getDeclaredConstructor"
            | "getDeclaredConstructors"
            | "getDeclaredField"
            | "getDeclaredFields"
            | "getDeclaredMethod"
            | "getDeclaredMethods"
            | "getField"
            | "getFields"
            | "getMethod"
            | "getMethods"
            | "newInstance"
    )
}

fn is_class_loader_method(name: &str) -> bool {
    matches!(name, "defineClass" | "findClass" | "loadClass")
}

fn is_runtime_bytecode_generation_owner(owner: &str) -> bool {
    owner.starts_with("org/objectweb/asm/")
        || owner.starts_with("javassist/")
        || owner.starts_with("net/bytebuddy/")
        || owner.starts_with("org/springframework/cglib/")
        || owner.starts_with("net/sf/cglib/")
}

fn collect_reference_types(reference: &MemberReference, types: &mut BTreeSet<String>) {
    if reference.owner != "<invokedynamic>" {
        types.insert(reference.owner.clone());
    }
    collect_descriptor_types(&reference.descriptor, types);
}

fn collect_descriptor_types(descriptor: &str, types: &mut BTreeSet<String>) {
    let mut chars = descriptor.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if ch != 'L' {
            continue;
        }

        let mut end = None;
        while let Some((index, value)) = chars.peek().copied() {
            chars.next();
            if value == ';' {
                end = Some(index);
                break;
            }
        }
        if let Some(end_index) = end {
            if let Some(start_index) = descriptor[..end_index].rfind('L') {
                let name = &descriptor[start_index + 1..end_index];
                if !name.is_empty() {
                    types.insert(name.to_owned());
                }
            }
        }
    }
}

fn classify(reason_codes: &BTreeSet<ClassificationReason>) -> PortingClassification {
    if reason_codes.iter().any(|reason| is_red_reason(*reason)) {
        PortingClassification::Red
    } else if reason_codes.iter().any(|reason| is_yellow_reason(*reason)) {
        PortingClassification::Yellow
    } else {
        PortingClassification::Green
    }
}

fn is_red_reason(reason: ClassificationReason) -> bool {
    matches!(
        reason,
        ClassificationReason::NativeMethod
            | ClassificationReason::UsesReflection
            | ClassificationReason::UsesCustomClassLoading
            | ClassificationReason::UsesUnsafe
            | ClassificationReason::LoadsNativeLibrary
            | ClassificationReason::UsesLegacySubroutine
            | ClassificationReason::UsesDynamicProxy
            | ClassificationReason::UsesRuntimeBytecodeGeneration
    )
}

fn is_yellow_reason(reason: ClassificationReason) -> bool {
    matches!(
        reason,
        ClassificationReason::SynchronizedMethod
            | ClassificationReason::UsesInvokeDynamic
            | ClassificationReason::UsesLambdaMetafactory
            | ClassificationReason::UsesSwitch
            | ClassificationReason::UsesMonitor
            | ClassificationReason::UsesExceptionHandlers
            | ClassificationReason::UsesJavaLangInvoke
            | ClassificationReason::AllocatesObjects
            | ClassificationReason::UsesVirtualDispatch
            | ClassificationReason::UsesArrays
            | ClassificationReason::ThrowsExceptions
            | ClassificationReason::ComplexBranching
    )
}

fn reason_text(reason: ClassificationReason) -> String {
    match reason {
        ClassificationReason::NoBytecodeBody => "method has no bytecode body".to_owned(),
        ClassificationReason::SimpleBytecode => {
            "bytecode inventory found no M1 difficulty features".to_owned()
        }
        ClassificationReason::NativeMethod => {
            "native method requires manual implementation".to_owned()
        }
        ClassificationReason::SynchronizedMethod => {
            "synchronized method may need ownership or concurrency redesign".to_owned()
        }
        ClassificationReason::UsesInvokeDynamic => {
            "invokedynamic requires dynamic invocation analysis".to_owned()
        }
        ClassificationReason::UsesLambdaMetafactory => {
            "lambda metafactory usage requires closure lowering review".to_owned()
        }
        ClassificationReason::UsesReflection => {
            "reflection API usage requires generated registry or manual design".to_owned()
        }
        ClassificationReason::UsesCustomClassLoading => {
            "custom class loading depends on JVM runtime behavior".to_owned()
        }
        ClassificationReason::UsesUnsafe => {
            "Unsafe API usage depends on JVM memory semantics".to_owned()
        }
        ClassificationReason::LoadsNativeLibrary => {
            "native library loading requires manual platform integration".to_owned()
        }
        ClassificationReason::UsesSwitch => {
            "switch bytecode needs structured control-flow recovery".to_owned()
        }
        ClassificationReason::UsesMonitor => {
            "monitor bytecode needs synchronization redesign".to_owned()
        }
        ClassificationReason::UsesLegacySubroutine => {
            "legacy jsr/ret bytecode is unsupported by early lowering".to_owned()
        }
        ClassificationReason::UsesExceptionHandlers => {
            "exception handlers require exception-control-flow analysis".to_owned()
        }
        ClassificationReason::UsesDynamicProxy => {
            "dynamic proxies require runtime dispatch replacement".to_owned()
        }
        ClassificationReason::UsesJavaLangInvoke => {
            "java.lang.invoke usage requires dynamic invocation review".to_owned()
        }
        ClassificationReason::UsesRuntimeBytecodeGeneration => {
            "runtime bytecode generation requires architectural replacement".to_owned()
        }
        ClassificationReason::AllocatesObjects => {
            "object allocation requires Java object-semantics review".to_owned()
        }
        ClassificationReason::UsesVirtualDispatch => {
            "virtual dispatch requires inheritance and dispatch review".to_owned()
        }
        ClassificationReason::UsesArrays => {
            "array bytecode requires Java array-semantics review".to_owned()
        }
        ClassificationReason::ThrowsExceptions => {
            "explicit throw bytecode requires exception lowering".to_owned()
        }
        ClassificationReason::ComplexBranching => {
            "many branch instructions require control-flow review".to_owned()
        }
    }
}

fn fixed_operand_width(opcode: u8) -> Option<usize> {
    let width = match opcode {
        0x10 | 0x15..=0x19 | 0x36..=0x3a | 0xbc => 1,
        0x11 | 0x84 | 0xbd | 0xc0 | 0xc1 => 2,
        0xc5 => 3,
        0xb9 | 0xba => 4,
        0x00..=0x0f
        | 0x1a..=0x35
        | 0x3b..=0x83
        | 0x85..=0x98
        | 0xac..=0xb1
        | 0xbe
        | 0xbf
        | 0xc2
        | 0xc3
        | 0xca
        | 0xfe
        | 0xff => 0,
        0x12 | 0x13 | 0x14 | 0x99..=0xab | 0xb2..=0xb8 | 0xbb | 0xc4 | 0xc6..=0xc9 => return None,
        _ => return None,
    };
    Some(width)
}

fn is_array_opcode(opcode: u8) -> bool {
    matches!(opcode, 0x2e..=0x35 | 0x4f..=0x56 | 0xbe)
}

fn opcode_name(opcode: u8) -> &'static str {
    match opcode {
        0x00 => "nop",
        0x01 => "aconst_null",
        0x02 => "iconst_m1",
        0x03 => "iconst_0",
        0x04 => "iconst_1",
        0x05 => "iconst_2",
        0x06 => "iconst_3",
        0x07 => "iconst_4",
        0x08 => "iconst_5",
        0x09 => "lconst_0",
        0x0a => "lconst_1",
        0x0b => "fconst_0",
        0x0c => "fconst_1",
        0x0d => "fconst_2",
        0x0e => "dconst_0",
        0x0f => "dconst_1",
        0x10 => "bipush",
        0x11 => "sipush",
        0x12 => "ldc",
        0x13 => "ldc_w",
        0x14 => "ldc2_w",
        0x15 => "iload",
        0x16 => "lload",
        0x17 => "fload",
        0x18 => "dload",
        0x19 => "aload",
        0x1a => "iload_0",
        0x1b => "iload_1",
        0x1c => "iload_2",
        0x1d => "iload_3",
        0x1e => "lload_0",
        0x1f => "lload_1",
        0x20 => "lload_2",
        0x21 => "lload_3",
        0x22 => "fload_0",
        0x23 => "fload_1",
        0x24 => "fload_2",
        0x25 => "fload_3",
        0x26 => "dload_0",
        0x27 => "dload_1",
        0x28 => "dload_2",
        0x29 => "dload_3",
        0x2a => "aload_0",
        0x2b => "aload_1",
        0x2c => "aload_2",
        0x2d => "aload_3",
        0x2e => "iaload",
        0x2f => "laload",
        0x30 => "faload",
        0x31 => "daload",
        0x32 => "aaload",
        0x33 => "baload",
        0x34 => "caload",
        0x35 => "saload",
        0x36 => "istore",
        0x37 => "lstore",
        0x38 => "fstore",
        0x39 => "dstore",
        0x3a => "astore",
        0x3b => "istore_0",
        0x3c => "istore_1",
        0x3d => "istore_2",
        0x3e => "istore_3",
        0x3f => "lstore_0",
        0x40 => "lstore_1",
        0x41 => "lstore_2",
        0x42 => "lstore_3",
        0x43 => "fstore_0",
        0x44 => "fstore_1",
        0x45 => "fstore_2",
        0x46 => "fstore_3",
        0x47 => "dstore_0",
        0x48 => "dstore_1",
        0x49 => "dstore_2",
        0x4a => "dstore_3",
        0x4b => "astore_0",
        0x4c => "astore_1",
        0x4d => "astore_2",
        0x4e => "astore_3",
        0x4f => "iastore",
        0x50 => "lastore",
        0x51 => "fastore",
        0x52 => "dastore",
        0x53 => "aastore",
        0x54 => "bastore",
        0x55 => "castore",
        0x56 => "sastore",
        0x57 => "pop",
        0x58 => "pop2",
        0x59 => "dup",
        0x5a => "dup_x1",
        0x5b => "dup_x2",
        0x5c => "dup2",
        0x5d => "dup2_x1",
        0x5e => "dup2_x2",
        0x5f => "swap",
        0x60 => "iadd",
        0x61 => "ladd",
        0x62 => "fadd",
        0x63 => "dadd",
        0x64 => "isub",
        0x65 => "lsub",
        0x66 => "fsub",
        0x67 => "dsub",
        0x68 => "imul",
        0x69 => "lmul",
        0x6a => "fmul",
        0x6b => "dmul",
        0x6c => "idiv",
        0x6d => "ldiv",
        0x6e => "fdiv",
        0x6f => "ddiv",
        0x70 => "irem",
        0x71 => "lrem",
        0x72 => "frem",
        0x73 => "drem",
        0x74 => "ineg",
        0x75 => "lneg",
        0x76 => "fneg",
        0x77 => "dneg",
        0x78 => "ishl",
        0x79 => "lshl",
        0x7a => "ishr",
        0x7b => "lshr",
        0x7c => "iushr",
        0x7d => "lushr",
        0x7e => "iand",
        0x7f => "land",
        0x80 => "ior",
        0x81 => "lor",
        0x82 => "ixor",
        0x83 => "lxor",
        0x84 => "iinc",
        0x85 => "i2l",
        0x86 => "i2f",
        0x87 => "i2d",
        0x88 => "l2i",
        0x89 => "l2f",
        0x8a => "l2d",
        0x8b => "f2i",
        0x8c => "f2l",
        0x8d => "f2d",
        0x8e => "d2i",
        0x8f => "d2l",
        0x90 => "d2f",
        0x91 => "i2b",
        0x92 => "i2c",
        0x93 => "i2s",
        0x94 => "lcmp",
        0x95 => "fcmpl",
        0x96 => "fcmpg",
        0x97 => "dcmpl",
        0x98 => "dcmpg",
        0x99 => "ifeq",
        0x9a => "ifne",
        0x9b => "iflt",
        0x9c => "ifge",
        0x9d => "ifgt",
        0x9e => "ifle",
        0x9f => "if_icmpeq",
        0xa0 => "if_icmpne",
        0xa1 => "if_icmplt",
        0xa2 => "if_icmpge",
        0xa3 => "if_icmpgt",
        0xa4 => "if_icmple",
        0xa5 => "if_acmpeq",
        0xa6 => "if_acmpne",
        0xa7 => "goto",
        0xa8 => "jsr",
        0xa9 => "ret",
        0xaa => "tableswitch",
        0xab => "lookupswitch",
        0xac => "ireturn",
        0xad => "lreturn",
        0xae => "freturn",
        0xaf => "dreturn",
        0xb0 => "areturn",
        0xb1 => "return",
        0xb2 => "getstatic",
        0xb3 => "putstatic",
        0xb4 => "getfield",
        0xb5 => "putfield",
        0xb6 => "invokevirtual",
        0xb7 => "invokespecial",
        0xb8 => "invokestatic",
        0xb9 => "invokeinterface",
        0xba => "invokedynamic",
        0xbb => "new",
        0xbc => "newarray",
        0xbd => "anewarray",
        0xbe => "arraylength",
        0xbf => "athrow",
        0xc0 => "checkcast",
        0xc1 => "instanceof",
        0xc2 => "monitorenter",
        0xc3 => "monitorexit",
        0xc4 => "wide",
        0xc5 => "multianewarray",
        0xc6 => "ifnull",
        0xc7 => "ifnonnull",
        0xc8 => "goto_w",
        0xc9 => "jsr_w",
        0xca => "breakpoint",
        0xfe => "impdep1",
        0xff => "impdep2",
        _ => "unknown",
    }
}

fn ensure_available(bytes: &[u8], offset: usize, length: usize) -> Result<(), String> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| format!("byte range at {offset} overflows"))?;
    if end > bytes.len() {
        Err(format!(
            "byte range {offset}..{end} exceeds buffer length {}",
            bytes.len()
        ))
    } else {
        Ok(())
    }
}

fn read_u8_at(bytes: &[u8], offset: usize) -> Result<u8, String> {
    ensure_available(bytes, offset, 1)?;
    Ok(bytes[offset])
}

fn read_u16_at(bytes: &[u8], offset: usize) -> Result<u16, String> {
    ensure_available(bytes, offset, 2)?;
    Ok(u16::from_be_bytes([bytes[offset], bytes[offset + 1]]))
}

fn read_i16_at(bytes: &[u8], offset: usize) -> Result<i16, String> {
    read_u16_at(bytes, offset).map(|value| value as i16)
}

fn read_i32_at(bytes: &[u8], offset: usize) -> Result<i32, String> {
    ensure_available(bytes, offset, 4)?;
    Ok(i32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]))
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn read_u8(&mut self) -> Result<u8, String> {
        let value = read_u8_at(self.bytes, self.position)?;
        self.position += 1;
        Ok(value)
    }

    fn read_u16(&mut self) -> Result<u16, String> {
        let value = read_u16_at(self.bytes, self.position)?;
        self.position += 2;
        Ok(value)
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        ensure_available(self.bytes, self.position, 4)?;
        let value = u32::from_be_bytes([
            self.bytes[self.position],
            self.bytes[self.position + 1],
            self.bytes[self.position + 2],
            self.bytes[self.position + 3],
        ]);
        self.position += 4;
        Ok(value)
    }

    fn read_bytes(&mut self, length: usize) -> Result<&'a [u8], String> {
        ensure_available(self.bytes, self.position, length)?;
        let start = self.position;
        self.position += length;
        Ok(&self.bytes[start..self.position])
    }

    fn skip(&mut self, length: usize) -> Result<(), String> {
        self.read_bytes(length).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_legacy_subroutine_opcode() {
        let mut collector = AnalysisCollector {
            instructions: Vec::new(),
            opcode_histogram: BTreeMap::new(),
            branches: Vec::new(),
            switches: Vec::new(),
            returns: Vec::new(),
            throws: Vec::new(),
            referenced_methods: BTreeSet::new(),
            referenced_fields: BTreeSet::new(),
            referenced_types: BTreeSet::new(),
            string_constants: BTreeSet::new(),
            features: BTreeSet::new(),
            reason_codes: BTreeSet::new(),
        };

        decode_code(
            &[0xa8, 0x00, 0x03, 0xb1],
            &empty_constant_pool(),
            &[],
            &mut collector,
        )
        .expect("jsr bytecode should decode");

        assert!(
            collector
                .features
                .contains(&BytecodeFeature::LegacySubroutine)
        );
        assert!(
            collector
                .reason_codes
                .contains(&ClassificationReason::UsesLegacySubroutine)
        );
    }

    #[test]
    fn rejects_truncated_branch_operands() {
        let mut collector = AnalysisCollector {
            instructions: Vec::new(),
            opcode_histogram: BTreeMap::new(),
            branches: Vec::new(),
            switches: Vec::new(),
            returns: Vec::new(),
            throws: Vec::new(),
            referenced_methods: BTreeSet::new(),
            referenced_fields: BTreeSet::new(),
            referenced_types: BTreeSet::new(),
            string_constants: BTreeSet::new(),
            features: BTreeSet::new(),
            reason_codes: BTreeSet::new(),
        };

        let error = decode_code(&[0x99, 0x00], &empty_constant_pool(), &[], &mut collector)
            .expect_err("truncated branch should fail");

        assert!(error.contains("exceeds buffer length"));
    }

    fn empty_constant_pool() -> ConstantPoolData {
        ConstantPoolData {
            entries: vec![ConstantPoolEntry::Empty],
        }
    }
}
