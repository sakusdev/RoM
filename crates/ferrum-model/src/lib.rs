//! Serializable models shared by Ferrum's importer, IR pipeline, and tools.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Increment this only when the JSON shape changes incompatibly.
pub const REPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JarReport {
    pub schema_version: u32,
    pub source: SourceInfo,
    pub manifest: Option<String>,
    pub summary: JarSummary,
    pub classes: Vec<ClassReport>,
    pub errors: Vec<EntryError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JarSummary {
    pub archive_entries: usize,
    pub class_entries_seen: usize,
    pub classes_parsed: usize,
    pub classes_failed: usize,
    pub fields: usize,
    pub methods: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassReport {
    /// Name as stored in the archive, such as `net/minecraft/Foo.class`.
    pub archive_path: String,
    /// JVM internal name, such as `net/minecraft/Foo`.
    pub internal_name: String,
    /// Human-readable dotted name, such as `net.minecraft.Foo`.
    pub dotted_name: String,
    pub super_name: Option<String>,
    pub interfaces: Vec<String>,
    pub version: ClassVersion,
    pub access: AccessInfo,
    pub constant_pool_entries: usize,
    pub attributes_count: usize,
    pub fields: Vec<MemberReport>,
    pub methods: Vec<MemberReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassVersion {
    pub java: u16,
    pub major: u16,
    pub minor: u16,
    pub preview: bool,
    pub display: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessInfo {
    pub bits: u16,
    pub debug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberReport {
    pub name: String,
    pub descriptor: String,
    pub access: AccessInfo,
    pub attributes_count: usize,
    /// Method bytecode inventory, present only when bytecode scanning is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytecode: Option<MethodBytecodeReport>,
}

/// Inventory extracted from one JVM method body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodBytecodeReport {
    /// Whether this method has a JVM `Code` attribute.
    pub has_code: bool,
    /// Raw byte length of the method's bytecode.
    pub code_length: Option<u32>,
    /// Maximum operand-stack depth declared by the class file.
    pub max_stack: Option<u16>,
    /// Maximum local-variable slots declared by the class file.
    pub max_locals: Option<u16>,
    /// Number of exception handlers declared by the `Code` attribute.
    pub exception_handler_count: usize,
    /// Exception table entries declared by the `Code` attribute.
    pub exception_table: Vec<ExceptionHandlerReport>,
    /// Line number table entries, when present.
    pub line_number_table: Vec<LineNumberReport>,
    /// Local variable table entries, when present.
    pub local_variable_table: Vec<LocalVariableReport>,
    /// Number of decoded JVM instructions.
    pub instruction_count: usize,
    /// Decoded opcode sequence with bytecode offsets.
    pub instructions: Vec<InstructionReport>,
    /// Deterministic opcode histogram keyed by mnemonic.
    pub opcode_histogram: BTreeMap<String, u64>,
    /// Branch instructions and their bytecode targets.
    pub branches: Vec<BranchInstructionReport>,
    /// Switch instructions and their bytecode targets.
    pub switches: Vec<SwitchInstructionReport>,
    /// Return instructions in this method body.
    pub returns: Vec<ReturnInstructionReport>,
    /// Explicit throw instructions in this method body.
    pub throws: Vec<ThrowInstructionReport>,
    /// Method references resolved from bytecode operands.
    pub referenced_methods: Vec<MemberReference>,
    /// Field references resolved from bytecode operands.
    pub referenced_fields: Vec<MemberReference>,
    /// Type references resolved from bytecode operands and handlers.
    pub referenced_types: Vec<String>,
    /// String constants loaded directly by this method.
    pub string_constants: Vec<String>,
    /// Machine-readable difficult JVM features detected in this method.
    pub features: Vec<BytecodeFeature>,
    /// Coarse porting difficulty classification.
    pub classification: PortingClassification,
    /// Machine-readable reasons that determined the classification.
    pub reason_codes: Vec<ClassificationReason>,
    /// Human-readable explanation of the classification.
    pub reasons: Vec<String>,
    /// Method-level control-flow graph, present only when CFG scanning is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cfg: Option<ControlFlowGraphReport>,
    /// Typed intermediate representation, present only when IR scanning is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ir: Option<MethodIrReport>,
}

/// Method-level control-flow graph recovered from JVM bytecode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlFlowGraphReport {
    /// Stable owner JVM internal class name.
    pub owner: String,
    /// Method name.
    pub method: String,
    /// JVM method descriptor.
    pub descriptor: String,
    /// Deterministic basic blocks ordered by block id.
    pub blocks: Vec<BasicBlockReport>,
    /// Exception handlers with block-level edges.
    pub exception_handlers: Vec<CfgExceptionHandlerReport>,
    /// Graph validation issues. Empty means branch/switch targets were valid.
    pub errors: Vec<CfgErrorReport>,
    /// Graphviz DOT representation of the CFG.
    pub dot: String,
}

/// A deterministic JVM bytecode basic block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicBlockReport {
    /// Stable zero-based block id.
    pub id: u32,
    /// Inclusive bytecode start offset.
    pub bytecode_start: u32,
    /// Exclusive bytecode end offset.
    pub bytecode_end: u32,
    /// Instructions contained in this block.
    pub instructions: Vec<InstructionReport>,
    /// Block terminator derived from the final instruction and successor offsets.
    pub terminator: TerminatorReport,
    /// Successor block ids, including exception-handler edges.
    pub successors: Vec<u32>,
}

/// A basic-block terminator recovered without typed stack analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TerminatorReport {
    /// Control falls through to the next block.
    Fallthrough { block: u32 },
    /// Conditional branch with explicit branch and fallthrough successors.
    Branch { then_block: u32, else_block: u32 },
    /// Unconditional branch.
    Goto { block: u32 },
    /// JVM switch terminator.
    Switch {
        cases: Vec<SwitchBlockCaseReport>,
        default_block: u32,
    },
    /// Method return.
    Return,
    /// Explicit throw.
    Throw,
    /// Block has no valid successor, usually because malformed bytecode was reported.
    Unreachable,
}

/// One block target in a switch terminator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchBlockCaseReport {
    /// Match value.
    pub value: i32,
    /// Target block id.
    pub block: u32,
}

/// Exception handler represented as block-level CFG edges.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfgExceptionHandlerReport {
    /// Inclusive protected bytecode start offset.
    pub start_pc: u16,
    /// Exclusive protected bytecode end offset.
    pub end_pc: u16,
    /// Handler bytecode offset.
    pub handler_pc: u16,
    /// Handler block id.
    pub handler_block: u32,
    /// Caught JVM internal type, or `None` for finally/all handlers.
    pub catch_type: Option<String>,
    /// Protected block ids that have exceptional edges to the handler.
    pub covered_blocks: Vec<u32>,
}

/// CFG validation error for malformed bytecode edges.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfgErrorReport {
    /// Bytecode offset where the invalid edge was found.
    pub offset: u32,
    /// Stable error code.
    pub code: CfgErrorCode,
    /// Human-readable explanation.
    pub message: String,
}

/// Machine-readable CFG validation error code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CfgErrorCode {
    InvalidBranchTarget,
    InvalidSwitchTarget,
    InvalidExceptionHandlerTarget,
}

/// Method descriptor parsed into explicit Java parameter and return types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodDescriptorReport {
    pub parameters: Vec<JavaTypeReport>,
    pub return_type: JavaTypeReport,
}

/// Field or local descriptor parsed into a structured Java type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JavaTypeReport {
    Byte,
    Char,
    Double,
    Float,
    Int,
    Long,
    Short,
    Boolean,
    Void,
    Null,
    Object {
        internal_name: String,
    },
    Array {
        dimensions: u8,
        component: Box<JavaTypeReport>,
    },
    Unknown {
        reason: String,
    },
}

impl JavaTypeReport {
    pub fn slot_width(&self) -> u16 {
        match self {
            JavaTypeReport::Long | JavaTypeReport::Double => 2,
            JavaTypeReport::Void => 0,
            _ => 1,
        }
    }
}

/// Parse a JVM method descriptor such as `(I)Ljava/lang/String;`.
pub fn parse_method_descriptor(descriptor: &str) -> Result<MethodDescriptorReport, String> {
    let mut parser = DescriptorParser::new(descriptor);
    parser.expect_byte(b'(')?;
    let mut parameters = Vec::new();
    while parser.peek_byte() != Some(b')') {
        parameters.push(parser.parse_type(false)?);
    }
    parser.expect_byte(b')')?;
    let return_type = parser.parse_type(true)?;
    parser.finish()?;
    Ok(MethodDescriptorReport {
        parameters,
        return_type,
    })
}

/// Parse a JVM field/local descriptor such as `I` or `[Ljava/lang/String;`.
pub fn parse_field_descriptor(descriptor: &str) -> Result<JavaTypeReport, String> {
    let mut parser = DescriptorParser::new(descriptor);
    let ty = parser.parse_type(false)?;
    parser.finish()?;
    Ok(ty)
}

struct DescriptorParser<'a> {
    descriptor: &'a [u8],
    cursor: usize,
}

impl<'a> DescriptorParser<'a> {
    fn new(descriptor: &'a str) -> Self {
        Self {
            descriptor: descriptor.as_bytes(),
            cursor: 0,
        }
    }

    fn peek_byte(&self) -> Option<u8> {
        self.descriptor.get(self.cursor).copied()
    }

    fn expect_byte(&mut self, expected: u8) -> Result<(), String> {
        match self.peek_byte() {
            Some(actual) if actual == expected => {
                self.cursor += 1;
                Ok(())
            }
            Some(actual) => Err(format!(
                "expected '{}' at byte {}, found '{}'",
                expected as char, self.cursor, actual as char
            )),
            None => Err(format!(
                "expected '{}' at end of descriptor",
                expected as char
            )),
        }
    }

    fn parse_type(&mut self, allow_void: bool) -> Result<JavaTypeReport, String> {
        let Some(tag) = self.peek_byte() else {
            return Err("unexpected end of descriptor".to_owned());
        };
        self.cursor += 1;
        match tag {
            b'B' => Ok(JavaTypeReport::Byte),
            b'C' => Ok(JavaTypeReport::Char),
            b'D' => Ok(JavaTypeReport::Double),
            b'F' => Ok(JavaTypeReport::Float),
            b'I' => Ok(JavaTypeReport::Int),
            b'J' => Ok(JavaTypeReport::Long),
            b'S' => Ok(JavaTypeReport::Short),
            b'Z' => Ok(JavaTypeReport::Boolean),
            b'V' if allow_void => Ok(JavaTypeReport::Void),
            b'V' => Err("void is not valid in a field or parameter descriptor".to_owned()),
            b'L' => self.parse_object_type(),
            b'[' => self.parse_array_type(),
            _ => Err(format!(
                "unsupported descriptor tag '{}' at byte {}",
                tag as char,
                self.cursor.saturating_sub(1)
            )),
        }
    }

    fn parse_object_type(&mut self) -> Result<JavaTypeReport, String> {
        let start = self.cursor;
        while let Some(value) = self.peek_byte() {
            self.cursor += 1;
            if value == b';' {
                let end = self.cursor - 1;
                if end == start {
                    return Err("empty object descriptor".to_owned());
                }
                let internal_name = std::str::from_utf8(&self.descriptor[start..end])
                    .map_err(|error| format!("object descriptor is not UTF-8: {error}"))?;
                return Ok(JavaTypeReport::Object {
                    internal_name: internal_name.to_owned(),
                });
            }
        }
        Err("unterminated object descriptor".to_owned())
    }

    fn parse_array_type(&mut self) -> Result<JavaTypeReport, String> {
        let mut dimensions = 1u8;
        while self.peek_byte() == Some(b'[') {
            self.cursor += 1;
            dimensions = dimensions
                .checked_add(1)
                .ok_or_else(|| "array descriptor has too many dimensions".to_owned())?;
        }
        let component = self.parse_type(false)?;
        if matches!(component, JavaTypeReport::Void) {
            return Err("array component cannot be void".to_owned());
        }
        Ok(JavaTypeReport::Array {
            dimensions,
            component: Box::new(component),
        })
    }

    fn finish(&self) -> Result<(), String> {
        if self.cursor == self.descriptor.len() {
            Ok(())
        } else {
            Err(format!(
                "trailing descriptor data starts at byte {}",
                self.cursor
            ))
        }
    }
}

/// Typed, SSA-like intermediate representation recovered from bytecode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodIrReport {
    /// Stable owner JVM internal class name.
    pub owner: String,
    /// Source method name.
    pub method: String,
    /// Raw JVM method descriptor.
    pub descriptor: String,
    /// Parsed method descriptor.
    pub parsed_descriptor: MethodDescriptorReport,
    /// Locals discovered from the descriptor, LocalVariableTable, and store inference.
    pub locals: Vec<IrLocalReport>,
    /// StackMapTable frames, when present. These are retained for merge validation.
    pub stack_map_frames: Vec<StackMapFrameReport>,
    /// Explicit-value IR instructions in deterministic bytecode order.
    pub instructions: Vec<IrInstructionReport>,
    /// Phi/merge placeholders inserted at CFG joins.
    pub merge_values: Vec<IrMergeValueReport>,
    /// Exception handlers retained as IR exception-control-flow metadata.
    pub exception_handlers: Vec<ExceptionHandlerReport>,
    /// Recoverable IR lowering issues. The surrounding JAR scan remains fault tolerant.
    pub errors: Vec<IrErrorReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrLocalReport {
    /// JVM local-variable slot.
    pub slot: u16,
    /// Source or generated local name.
    pub name: Option<String>,
    /// Best-known local type.
    pub ty: JavaTypeReport,
    /// JVM slot width for the type.
    pub slot_width: u16,
    /// Where the local type/name came from.
    pub source: IrLocalSource,
    /// Inclusive bytecode scope start when known.
    pub start_pc: Option<u16>,
    /// Exclusive bytecode scope end when known.
    pub end_pc: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IrLocalSource {
    This,
    MethodParameter,
    LocalVariableTable,
    StoreInference,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrInstructionReport {
    /// Stable zero-based instruction id within the IR.
    pub id: u32,
    /// Source provenance for this IR node.
    pub source: IrSourceReport,
    #[serde(flatten)]
    pub kind: IrInstructionKindReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrSourceReport {
    pub class: String,
    pub method: String,
    pub descriptor: String,
    pub bytecode_offset: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_number: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrValueReport {
    pub id: u32,
    pub ty: JavaTypeReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IrInstructionKindReport {
    Constant {
        output: IrValueReport,
        value: ConstantValueReport,
    },
    LoadLocal {
        output: IrValueReport,
        local: u16,
    },
    StoreLocal {
        local: u16,
        value: u32,
    },
    LoadField {
        output: IrValueReport,
        object: Option<u32>,
        field: MemberReference,
    },
    StoreField {
        object: Option<u32>,
        field: MemberReference,
        value: u32,
    },
    Binary {
        output: IrValueReport,
        operation: String,
        left: u32,
        right: u32,
    },
    Unary {
        output: IrValueReport,
        operation: String,
        input: u32,
    },
    Convert {
        output: IrValueReport,
        operation: String,
        input: u32,
    },
    Invoke {
        output: Option<IrValueReport>,
        invocation: InvocationKindReport,
        target: MemberReference,
        receiver: Option<u32>,
        arguments: Vec<u32>,
    },
    NewObject {
        output: IrValueReport,
        class: String,
    },
    NewArray {
        output: IrValueReport,
        component_type: JavaTypeReport,
        dimensions: Vec<u32>,
    },
    Cast {
        output: IrValueReport,
        input: u32,
        target_type: JavaTypeReport,
    },
    InstanceOf {
        output: IrValueReport,
        input: u32,
        target_type: JavaTypeReport,
    },
    ArrayLoad {
        output: IrValueReport,
        array: u32,
        index: u32,
    },
    ArrayStore {
        array: u32,
        index: u32,
        value: u32,
    },
    ArrayLength {
        output: IrValueReport,
        array: u32,
    },
    Branch {
        opcode: String,
        condition_values: Vec<u32>,
        target: i32,
        fallthrough: Option<u32>,
    },
    Switch {
        input: u32,
        default_target: i32,
        cases: Vec<SwitchCaseReport>,
    },
    Phi {
        output: IrValueReport,
        block: u32,
        inputs: Vec<u32>,
    },
    Return {
        value: Option<u32>,
    },
    Throw {
        value: u32,
    },
    Monitor {
        operation: MonitorOperationReport,
        object: u32,
    },
    LocalIncrement {
        local: u16,
        amount: i16,
    },
    StackOperation {
        operation: String,
        values: Vec<u32>,
    },
    Unsupported {
        opcode: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConstantValueReport {
    Null,
    Int { value: i32 },
    Long { value: i64 },
    Float { value: f32 },
    Double { value: f64 },
    String { value: String },
    Class { descriptor: String },
    Unknown { description: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvocationKindReport {
    Static,
    Special,
    Virtual,
    Interface,
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitorOperationReport {
    Enter,
    Exit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrMergeValueReport {
    pub block: u32,
    pub bytecode_offset: u32,
    pub output: IrValueReport,
    pub inputs: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrErrorReport {
    pub offset: Option<u32>,
    pub code: IrErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IrErrorCode {
    DescriptorParseFailed,
    StackUnderflow,
    InvalidLocal,
    UnsupportedOpcode,
    UnsupportedStackOperation,
    ConservativeMerge,
    ExceptionEdgePreserved,
    StackMapTableMalformed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackMapFrameReport {
    pub index: u16,
    pub frame_type: u8,
    pub kind: String,
    pub offset_delta: u16,
    pub bytecode_offset: u32,
    pub locals: Vec<StackMapVerificationTypeReport>,
    pub stack: Vec<StackMapVerificationTypeReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StackMapVerificationTypeReport {
    Top,
    Integer,
    Float,
    Double,
    Long,
    Null,
    UninitializedThis,
    Object { class: String },
    Uninitialized { offset: u16 },
}

/// One entry in a method exception table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExceptionHandlerReport {
    /// Inclusive start bytecode offset for the protected range.
    pub start_pc: u16,
    /// Exclusive end bytecode offset for the protected range.
    pub end_pc: u16,
    /// Handler bytecode offset.
    pub handler_pc: u16,
    /// Caught JVM internal type, or `None` for finally/all handlers.
    pub catch_type: Option<String>,
}

/// One entry in a JVM `LineNumberTable` attribute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineNumberReport {
    /// Bytecode offset where this source line starts.
    pub start_pc: u16,
    /// Source line number.
    pub line_number: u16,
}

/// One entry in a JVM `LocalVariableTable` attribute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalVariableReport {
    /// Bytecode offset where the local variable is in scope.
    pub start_pc: u16,
    /// Scope length in bytes.
    pub length: u16,
    /// Local-variable slot index.
    pub index: u16,
    /// Local-variable source name.
    pub name: String,
    /// JVM descriptor for the local variable.
    pub descriptor: String,
}

/// Decoded JVM instruction mnemonic at a bytecode offset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionReport {
    /// Bytecode offset of the instruction.
    pub offset: u32,
    /// JVM opcode mnemonic.
    pub opcode: String,
}

/// Decoded JVM branch instruction target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInstructionReport {
    /// Bytecode offset of the branch instruction.
    pub offset: u32,
    /// JVM branch opcode mnemonic.
    pub opcode: String,
    /// Absolute bytecode target offset.
    pub target: i32,
}

/// Decoded JVM switch instruction target table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchInstructionReport {
    /// Bytecode offset of the switch instruction.
    pub offset: u32,
    /// JVM switch opcode mnemonic.
    pub opcode: String,
    /// Absolute bytecode target offset for the default case.
    pub default_target: i32,
    /// Deterministic case table in bytecode order.
    pub cases: Vec<SwitchCaseReport>,
}

/// One case in a decoded JVM switch instruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchCaseReport {
    /// Match value.
    pub value: i32,
    /// Absolute bytecode target offset.
    pub target: i32,
}

/// Decoded JVM return instruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnInstructionReport {
    /// Bytecode offset of the return instruction.
    pub offset: u32,
    /// JVM return opcode mnemonic.
    pub opcode: String,
}

/// Decoded JVM throw instruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrowInstructionReport {
    /// Bytecode offset of the `athrow` instruction.
    pub offset: u32,
}

/// A resolved field or method reference from the constant pool.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MemberReference {
    /// JVM internal owner type.
    pub owner: String,
    /// Member name.
    pub name: String,
    /// JVM member descriptor.
    pub descriptor: String,
}

/// Difficult JVM features detected while inventorying bytecode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BytecodeFeature {
    NativeMethod,
    SynchronizedMethod,
    InvokeDynamic,
    LambdaMetafactory,
    ReflectionApi,
    CustomClassLoader,
    UnsafeApi,
    NativeLibraryLoading,
    SwitchInstruction,
    MonitorEnter,
    MonitorExit,
    LegacySubroutine,
    ExceptionHandlers,
    DynamicProxy,
    JavaLangInvoke,
    RuntimeBytecodeGeneration,
    ObjectAllocation,
    VirtualDispatch,
    ArrayOperation,
    ThrowInstruction,
}

/// Coarse classification for likely porting difficulty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortingClassification {
    Green,
    Yellow,
    Red,
}

/// Machine-readable reason contributing to a porting classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassificationReason {
    NoBytecodeBody,
    SimpleBytecode,
    NativeMethod,
    SynchronizedMethod,
    UsesInvokeDynamic,
    UsesLambdaMetafactory,
    UsesReflection,
    UsesCustomClassLoading,
    UsesUnsafe,
    LoadsNativeLibrary,
    UsesSwitch,
    UsesMonitor,
    UsesLegacySubroutine,
    UsesExceptionHandlers,
    UsesDynamicProxy,
    UsesJavaLangInvoke,
    UsesRuntimeBytecodeGeneration,
    AllocatesObjects,
    UsesVirtualDispatch,
    UsesArrays,
    ThrowsExceptions,
    ComplexBranching,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryError {
    pub archive_path: String,
    pub stage: ErrorStage,
    pub message: String,
    /// Best-effort header information, available even when full parsing fails.
    pub classfile_major: Option<u16>,
    pub classfile_minor: Option<u16>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorStage {
    ArchiveRead,
    ClassParse,
    ClassVerify,
    ConstantPoolResolve,
    BytecodeInventory,
}
