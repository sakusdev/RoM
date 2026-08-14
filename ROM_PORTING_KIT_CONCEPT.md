# RoM Porting Kit — Project Concept and Codex Handoff

## 0. Document purpose

This document defines the concept, architecture, current status, development policy, and immediate implementation tasks for **RoM Porting Kit**.

RoM Porting Kit is an experimental Rust toolchain intended to accelerate the clean and testable reimplementation of Minecraft Java Edition server behavior in Rust.

It is **not** a promise of perfect one-click conversion from `server.jar` to idiomatic Rust. Its role is to automate the repetitive and mechanically verifiable parts of porting so that developers can focus on Minecraft-specific behavior, architecture, compatibility, and performance.

---

# 1. Project vision

The long-term target is a Rust-native Minecraft Java Edition compatible server with the following properties:

- No mandatory JVM in native mode
- Native binaries for Windows, Linux, macOS, and ARM platforms
- Low idle memory usage
- Fast startup
- Region-based parallel ticking
- Deterministic game-state updates
- Existing Java Edition client compatibility
- Existing Anvil/NBT world compatibility
- Native Rust and sandboxed WASM extension APIs
- Optional, partial Fabric compatibility through analysis and bridge layers

The project is split into two major products:

1. **RoM Porting Kit**
   - JAR and class-file analysis
   - Mapping resolution
   - Bytecode inventory
   - Control-flow reconstruction
   - Typed intermediate representation
   - Rust skeleton generation
   - Minecraft-specific rewrite rules
   - Mixin and Fabric compatibility analysis
   - Differential testing support

2. **RoM Server**
   - An independently structured Rust server implementation
   - Uses knowledge, tests, generated models, and reports produced by the porting kit
   - Does not depend on mechanically publishing Mojang source converted into Rust

---

# 2. Core principle

The project must not be designed as a naive transpiler:

```text
server.jar -> decompile -> syntax replacement -> Rust
```

That approach fails because Java and Rust have fundamentally different semantics:

- Garbage-collected shared references versus explicit ownership
- Java exceptions versus Rust `Result`
- Java inheritance versus Rust composition, enums, and traits
- JVM monitors versus ownership-based concurrency
- Reflection and class loading versus generated registries
- Mutable object graphs versus handles, arenas, or region ownership
- Stack-machine bytecode versus structured Rust control flow

The intended pipeline is:

```text
JAR / CLASS input
        |
        v
Class-file and metadata importer
        |
        v
Mappings and symbol normalization
        |
        v
Bytecode inventory
        |
        v
Control-flow graph
        |
        v
Typed SSA-like intermediate representation
        |
        v
Java semantic lowering
        |
        v
Minecraft-specific rewrite rules
        |
        v
Rust skeleton and implementation assistance
        |
        v
Differential validation against Java behavior
```

The generated code is initially allowed to be conservative and mechanical. Human developers then replace compatibility-oriented structures with idiomatic, efficient Rust implementations.

---

# 3. Success criteria

RoM is successful even before full source conversion exists.

The first meaningful success condition is:

> Given a Minecraft server JAR or Fabric Mod JAR, RoM can produce a reliable inventory showing which code can probably be ported automatically, which code requires review, and which code requires manual redesign.

Later success conditions are:

1. Generate valid Rust type and method skeletons from selected Java classes.
2. Recover structured control flow for common JVM methods.
3. Convert simple pure functions into executable Rust.
4. Compare Java and Rust results automatically.
5. Port selected Minecraft data structures with verified equivalent behavior.
6. Use the toolchain to accelerate construction of a Rust-native server.

---

# 4. Explicit non-goals

The project must not initially attempt the following:

- Fully automatic conversion of all of `server.jar`
- Perfect idiomatic Rust generation
- Complete JVM emulation
- Complete Java standard-library emulation
- Complete Fabric Loader compatibility
- Complete Mixin compatibility
- Immediate support for all Minecraft versions
- Immediate support for all mappings
- Publishing Mojang-owned converted source code
- Reproducing every vanilla bug before a functional server exists
- Treating decompiler output as the authoritative source of semantics

The initial target should be one specific Minecraft Java Edition server version.

---

# 5. Legal and distribution boundary

RoM should be distributed as original tooling, runtime support, rewrite rules, tests, and independently written server code.

A safe project layout is:

```text
Public repository
  - RoM source code
  - Generic JVM analysis code
  - Original Rust runtime code
  - Rewrite and mapping configuration
  - Test infrastructure
  - Documentation

User machine
  - Legitimately obtained server.jar
  - Optional mappings
  - Locally generated reports or temporary code
```

Do not commit or publish:

- Mojang server JARs
- Large mechanically converted Mojang source trees
- Decompiled Minecraft source
- Copyrighted assets

Generated code should preserve source provenance for local debugging, but public outputs should remain independently authored or limited to metadata, patches, tests, and clean-room-compatible abstractions.

This document is not legal advice.

---

# 6. Current repository state

The current workspace contains an M0 implementation.

```text
rom-porting-kit/
├─ crates/
│  ├─ rom-model/
│  ├─ rom-importer/
│  └─ rom-cli/
├─ docs/
│  ├─ ARCHITECTURE.md
│  └─ ROADMAP.md
├─ fixtures/sample/
├─ tools/reference_scan.py
├─ Cargo.toml
├─ rust-toolchain.toml
└─ README.md
```

Current package responsibilities:

## `rom-model`

Shared serializable data contracts.

Current models include:

- `JarReport`
- `SourceInfo`
- `JarSummary`
- `ClassReport`
- `ClassVersion`
- `AccessInfo`
- `MemberReport`
- `EntryError`
- `ErrorStage`

Reports currently use:

```text
schema_version: 1
```

Schema versions must be incremented only for incompatible report changes.

## `rom-importer`

Current responsibilities:

- Open ZIP/JAR archives
- Read `META-INF/MANIFEST.MF`
- Detect `.class` entries
- Ignore `module-info.class`
- Filter by class prefix
- Apply an optional class scan limit
- Parse class files using `ristretto_classfile`
- Optionally verify class-file structure
- Resolve class, superclass, interface, field, and method names
- Extract JVM descriptors
- Preserve per-entry errors instead of aborting the entire scan

## `rom-cli`

Current command:

```bash
ferrum inspect <JAR>
```

Supported options:

```text
-o, --output <PATH>
--prefix <PREFIX>
--limit <COUNT>
--verify
--compact
--fail-on-class-error
```

Example:

```bash
cargo build --release -p ferrum

./target/release/ferrum inspect server.jar \
  --prefix net.minecraft \
  --verify \
  -o server-report.json
```

---

# 7. M0 — implemented JAR inventory

M0 provides a fault-tolerant archive and class metadata scanner.

Current report data includes:

- Archive path
- JVM internal class name
- Dotted class name
- Superclass
- Interfaces
- Class-file version
- Java version
- Preview flag
- Access flags
- Constant-pool entry count
- Attribute count
- Fields
- Field descriptors
- Methods
- Method descriptors
- Per-entry parse, verification, and resolution errors
- Aggregate class, field, and method counts

Design requirement:

> A malformed or unsupported class must not normally abort analysis of the entire JAR.

Strict CI behavior can be requested through `--fail-on-class-error`.

---

# 8. Immediate next milestone: M1 bytecode inventory

The next task is to inspect method bodies and classify porting difficulty.

## M1 goals

For every method with a `Code` attribute, extract:

- Bytecode length
- Maximum operand-stack size
- Maximum local-variable count
- Exception table
- Line number table when available
- Local variable table when available
- Opcode sequence
- Opcode histogram
- Branch instructions
- Switch instructions
- Return and throw instructions
- Method and field references
- Type references
- String constants where useful

Detect difficult JVM features:

- `invokedynamic`
- Lambda metafactory usage
- Reflection APIs
- Custom class loaders
- JNI or `native` methods
- `synchronized` methods
- `monitorenter`
- `monitorexit`
- `jsr` and `ret` legacy bytecode
- Exception-heavy control flow
- Dynamic proxies
- `java.lang.invoke`
- `sun.misc.Unsafe`
- `jdk.internal.misc.Unsafe`
- Runtime bytecode generation libraries
- Native library loading

## M1 output

Extend method reports with bytecode metadata and classification.

Suggested model:

```rust
pub struct MethodBytecodeReport {
    pub has_code: bool,
    pub code_length: Option<u32>,
    pub max_stack: Option<u16>,
    pub max_locals: Option<u16>,
    pub exception_handler_count: usize,
    pub instruction_count: usize,
    pub opcode_histogram: BTreeMap<String, u64>,
    pub referenced_methods: Vec<MemberReference>,
    pub referenced_fields: Vec<MemberReference>,
    pub referenced_types: Vec<String>,
    pub features: Vec<BytecodeFeature>,
    pub classification: PortingClassification,
    pub reasons: Vec<String>,
}
```

Suggested classification:

```rust
pub enum PortingClassification {
    Green,
    Yellow,
    Red,
}
```

## Initial classification policy

### Green

Likely suitable for early automatic conversion:

- No bytecode body, or a simple body
- Primitive arithmetic
- Local variables
- Field access
- Straightforward conditionals
- Simple loops
- Static method calls with known mappings
- No reflection
- No monitor instructions
- No native methods
- No complex exception control flow

### Yellow

Can be analyzed and partially generated, but requires review:

- Complex branching
- Switch expressions
- Object allocation
- Virtual dispatch
- Arrays and collections
- Multiple exception handlers
- Lambdas or `invokedynamic`
- Generic Java object semantics
- Shared mutable references
- Inheritance-heavy code
- Synchronization that may be redesigned

### Red

Requires manual implementation or architectural replacement:

- Native methods
- Reflection-heavy code
- Custom class loading
- Runtime bytecode generation
- Unsafe memory access
- Arbitrary Mixin bytecode transformation
- Deep dependence on JVM implementation details
- Unresolvable dynamic invocation

Classification must always include machine-readable reasons.

---

# 9. M2 — control-flow graph

After M1, build method-level control-flow graphs.

## Required types

```rust
pub struct MethodBody {
    pub owner: ClassId,
    pub name: String,
    pub descriptor: MethodDescriptor,
    pub blocks: Vec<BasicBlock>,
    pub exception_handlers: Vec<ExceptionHandler>,
}

pub struct BasicBlock {
    pub id: BlockId,
    pub bytecode_start: u32,
    pub instructions: Vec<StackInstruction>,
    pub terminator: Terminator,
}

pub enum Terminator {
    Fallthrough(BlockId),
    Branch {
        condition: StackValue,
        then_block: BlockId,
        else_block: BlockId,
    },
    Switch {
        value: StackValue,
        cases: Vec<(i32, BlockId)>,
        default: BlockId,
    },
    Return(Option<StackValue>),
    Throw(StackValue),
    Unreachable,
}
```

## CFG requirements

- Deterministic block IDs
- Stable JSON serialization
- Branch targets validated
- Exception-handler edges represented
- `tableswitch` and `lookupswitch` supported
- Graphviz DOT output
- Malformed methods reported without crashing the JAR scan

---

# 10. M3 — typed intermediate representation

JVM bytecode is stack based. Rust generation must not operate directly on raw push/pop instructions.

M3 converts stack-machine bytecode to a typed SSA-like IR.

## Required stages

1. Parse field and method descriptors.
2. Determine local variable types.
3. Simulate the JVM operand stack.
4. Use `StackMapTable` when present.
5. Insert merge values or phi nodes at control-flow joins.
6. Convert stack operations into explicit values.
7. Preserve exception semantics.

Suggested instruction model:

```rust
pub enum IrInstruction {
    Constant {
        output: ValueId,
        value: ConstantValue,
    },
    LoadLocal {
        output: ValueId,
        local: LocalId,
    },
    StoreLocal {
        local: LocalId,
        value: ValueId,
    },
    LoadField {
        output: ValueId,
        object: ValueId,
        field: FieldId,
    },
    StoreField {
        object: ValueId,
        field: FieldId,
        value: ValueId,
    },
    Binary {
        output: ValueId,
        operation: BinaryOperation,
        left: ValueId,
        right: ValueId,
    },
    Invoke {
        output: Option<ValueId>,
        kind: InvocationKind,
        target: MethodId,
        receiver: Option<ValueId>,
        arguments: Vec<ValueId>,
    },
    NewObject {
        output: ValueId,
        class: ClassId,
    },
    Cast {
        output: ValueId,
        input: ValueId,
        target: JavaType,
    },
}
```

Every IR node must preserve source provenance:

- Source class
- Source method
- Descriptor
- Bytecode offset
- Optional line number

---

# 11. M4 — Rust skeleton generation

M4 should first generate compilable structure, not perfect implementations.

## Basic mappings

```text
Java class       -> Rust struct + impl
Java interface   -> Rust trait or capability
Java enum        -> Rust enum where semantics permit
Java null        -> Option<T>
Java array       -> Vec<T>, Box<[T]>, or specialized array type
Java exception   -> Result<T, E> or explicit panic only where justified
Java static      -> const, static, OnceLock, or registry initialization
Java reference   -> handle, arena key, Arc, Rc, or owned value based on policy
```

## Generated source requirements

Generated files must:

- Be deterministic
- Be rustfmt-compatible
- Retain source provenance
- Use stable generated symbol names
- Insert `todo!()` for unsupported method bodies
- Emit warnings as structured metadata, not only comments
- Avoid silently changing behavior

Example:

```rust
#[rom_source(
    class = "net/minecraft/util/math/BlockPos",
    method = "offset",
    descriptor = "(III)Lnet/minecraft/util/math/BlockPos;"
)]
pub fn offset(&self, x: i32, y: i32, z: i32) -> BlockPos {
    todo!("method body has not yet been lowered")
}
```

---

# 12. Java semantic compatibility layer

An initial compatibility layer may be used to get generated code running before optimization.

Possible temporary abstractions:

- `JavaObjectId`
- Object arenas
- Nullable handles
- Java-compatible numeric conversion helpers
- Java string helpers
- Java collection adapters
- Java exception values
- Monitor wrappers
- Java identity-hash behavior where required

This compatibility layer is transitional.

Long-term optimization should replace:

```text
Shared mutable Java graph -> region ownership and handles
Java inheritance          -> composition, enums, and traits
Reflection                -> generated registries
Monitors                   -> explicit ownership and message passing
Exceptions                -> typed errors
Dynamic lookup            -> generated match tables or registries
```

---

# 13. Minecraft-specific rewrite system

Generic JVM conversion is insufficient. RoM requires a configurable rewrite system.

Suggested configuration files:

```text
mappings/
├─ types.toml
├─ methods.toml
├─ fields.toml
├─ constructors.toml
├─ ownership.toml
├─ manual-overrides.toml
└─ minecraft-special-cases.toml
```

Example type mappings:

```toml
[java_types]
"java/lang/String" = "String"
"java/util/UUID" = "uuid::Uuid"
"net/minecraft/util/Identifier" = "rom_types::Identifier"
"net/minecraft/util/math/BlockPos" = "rom_types::BlockPos"
"net/minecraft/util/math/ChunkPos" = "rom_types::ChunkPos"
"net/minecraft/item/ItemStack" = "rom_inventory::ItemStack"
```

Example method mappings:

```toml
[java_methods]
"java/lang/Math.floor(D)D" = "f64::floor"
"java/lang/System.nanoTime()J" = "rom_time::monotonic_nanos"
"java/util/Objects.requireNonNull(Ljava/lang/Object;)Ljava/lang/Object;" = "rom_compat::require_non_null"
```

Minecraft structures that deserve dedicated handlers:

- `Identifier`
- `RegistryKey`
- Registry systems
- `Codec`
- `PacketCodec`
- `PacketByteBuf`
- `BlockPos`
- `ChunkPos`
- `Vec3d`
- `BlockState`
- `ItemStack`
- NBT types
- Paletted containers
- Chunk sections
- Data trackers
- Scheduled ticks
- Resource loading
- Tags
- Recipes
- Loot tables

---

# 14. Mapping support

RoM should support symbol mapping as a separate layer above generic JVM import.

Potential namespaces:

- Official/Mojang names
- Intermediary names
- Yarn named mappings

Internally, symbols should be identified by stable structured IDs rather than display names alone.

Suggested model:

```rust
pub struct SymbolNameSet {
    pub official: Option<String>,
    pub intermediary: Option<String>,
    pub named: Option<String>,
    pub selected: String,
}
```

Mapping changes between Minecraft versions should be diffable.

---

# 15. Fabric and Mixin analysis

RoM may analyze Fabric Mod JARs, but complete Fabric compatibility is not an immediate target.

## Fabric metadata inspection

Analyze:

- `fabric.mod.json`
- Mod ID
- Version
- Environment
- Entrypoints
- Dependencies
- Nested JARs
- Access wideners
- Mixin configuration files
- Client-only versus server-compatible code

## Mixin compatibility analysis

For each Mixin, identify:

- Target class
- Target method
- Injection type
- Injection point
- Cancellable behavior
- Local capture
- Shadow fields and methods
- Accessors and invokers
- Redirects
- Variable or constant modification

Suggested classification:

```text
SUPPORTED
PARTIALLY_SUPPORTED
SEMANTIC_REWRITE_REQUIRED
UNSUPPORTED
```

Possible early conversions:

```text
@Inject at HEAD   -> pre-hook event
@Inject at RETURN -> post-hook event
Simple cancellable callback -> hook with result override
Accessor          -> generated facade accessor
Invoker           -> generated facade method
```

Hard or unsupported cases:

- Arbitrary bytecode `Redirect`
- Deep local-variable capture
- Expression modification at unstable instruction offsets
- Mixins depending on exact Java class layout
- Runtime-generated Mixins

The goal is initially to generate a useful compatibility report, not to execute arbitrary Mixins.

---

# 16. Differential testing

Differential testing is a first-class feature, not a final cleanup step.

The same operation should be executable in Java and Rust, then compared.

```text
Input corpus
  |-- Java reference runner -> Java result
  `-- Rust implementation  -> Rust result
                              |
                              v
                         Comparator
```

Compare:

- Return values
- Exceptions or typed errors
- Changed fields
- NBT output
- Packet output
- World state
- Entity state
- Inventory state
- Scheduled ticks
- Random number generator state where relevant

## First pilot types

Recommended early ports:

1. `Direction`
2. `BlockPos`
3. `ChunkPos`
4. `Vec3d`
5. `Identifier`
6. Primitive NBT tags
7. Packet VarInt/VarLong utilities
8. Paletted integer helpers

These types have relatively contained behavior and provide good coverage of arithmetic, enums, bit packing, and serialization.

## Replay format

A replay file should be versioned and deterministic.

```rust
pub struct ReplayCase {
    pub schema_version: u32,
    pub target: SymbolId,
    pub seed: Option<u64>,
    pub inputs: Vec<TestValue>,
    pub expected: TestOutcome,
}
```

---

# 17. Relationship to the future Rust server

RoM Porting Kit is not itself the final server.

The server should use an architecture suitable for Rust rather than preserve the vanilla Java object graph.

Preferred server architecture:

```text
Minecraft client
      |
      v
Protocol adapters
      |
      v
Canonical server model
      |
      v
Global tick coordinator
  |-- Region worker 0
  |-- Region worker 1
  |-- Region worker 2
  `-- Region worker N
      |
      |-- async networking
      |-- async compression
      |-- async chunk I/O
      |-- async persistence
      `-- async world-generation jobs
```

Game-state writes should be deterministic and owned by a region worker.

Cross-region changes should use messages rather than uncontrolled shared mutation.

Example:

```rust
pub enum RegionMessage {
    TransferEntity(EntitySnapshot),
    SetBlock {
        position: BlockPos,
        state: BlockStateId,
    },
    ScheduleTick(ScheduledTick),
    ApplyExplosion(ExplosionRequest),
}
```

RoM-generated knowledge and tests should help implement this architecture, not prevent it.

---

# 18. Recommended future workspace

As the project expands, use this structure:

```text
rom-porting-kit/
├─ crates/
│  ├─ rom-model/
│  ├─ rom-importer/
│  ├─ rom-bytecode/
│  ├─ rom-cfg/
│  ├─ rom-ir/
│  ├─ rom-mappings/
│  ├─ rom-analysis/
│  ├─ rom-codegen-rust/
│  ├─ rom-minecraft-rules/
│  ├─ rom-fabric/
│  ├─ rom-mixin/
│  ├─ rom-diff/
│  └─ rom-cli/
├─ runtime/
│  ├─ rom-java-compat/
│  ├─ rom-object-arena/
│  └─ rom-test-runtime/
├─ mappings/
├─ fixtures/
├─ tests/
├─ tools/
└─ docs/
```

Do not create every crate immediately. Split a crate only when the boundary is clear and independently testable.

---

# 19. Engineering rules

## Reliability

- Never trust class-file offsets without bounds checking.
- A bad class or method must not normally abort a full archive scan.
- Avoid panics for user-controlled input.
- Report errors with archive path, class, method, descriptor, and bytecode offset.
- Keep output deterministic.

## Performance

- Support large JARs without loading the entire archive into memory.
- Read one class entry at a time.
- Avoid retaining raw bytecode unless requested.
- Use streaming or indexed reports where practical.
- Add optional parallel class analysis only after deterministic output is guaranteed.

## Compatibility

- Keep generic JVM analysis separate from Minecraft-specific behavior.
- Do not hard-code one mapping namespace into the importer.
- Preserve JVM internal names as canonical identifiers.
- Treat dotted names as presentation only.

## Testing

Every new bytecode feature must have:

- A minimal Java source fixture
- A compiled JAR or reproducible fixture build script
- An importer/analysis test
- A malformed-input test where relevant
- Stable JSON snapshot or direct assertions

## Code quality

Before considering a task complete:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Avoid unnecessary `unsafe`. Any required `unsafe` block must document its invariant.

---

# 20. CLI evolution

Current:

```bash
ferrum inspect server.jar
```

Planned commands:

```bash
ferrum inspect server.jar
ferrum bytecode server.jar
ferrum classify server.jar
ferrum cfg server.jar --class <CLASS> --method <METHOD>
ferrum map server.jar --mappings yarn.tiny
ferrum generate server.jar --class <CLASS> --output generated/
ferrum fabric inspect example-mod.jar
ferrum diff run replay.json
```

Do not overload `inspect` with every future concern. Prefer explicit subcommands and versioned output schemas.

---

# 21. Development roadmap

## M0 — JAR inventory

Status: initial implementation exists.

- Archive reading
- Class metadata parsing
- Descriptor extraction
- Fault-tolerant errors
- JSON report
- Prefix filter
- Scan limit
- Optional class verification

## M1 — bytecode inventory

Next milestone.

- Decode `Code` attributes
- Enumerate instructions
- Opcode statistics
- Reference extraction
- Difficulty feature detection
- Green/Yellow/Red classification

## M2 — control-flow graph

- Basic blocks
- Branch edges
- Switch edges
- Exception edges
- DOT and JSON output

## M3 — typed IR

- Descriptor parser
- Stack simulation
- Stack-map handling
- SSA-like values
- Phi or merge values

## M4 — Rust skeleton generation

- Types
- Fields
- Method signatures
- Provenance attributes
- Unsupported body placeholders

## M5 — Minecraft rules

- Mapping ingestion
- Rewrite TOML
- Dedicated Minecraft type handlers
- Initial pilot ports

## M6 — differential tests

- Java harness
- Rust harness
- Randomized corpus
- Replay files
- Result comparator

## M7 — Fabric and Mixin analyzer

- Fabric metadata
- Access wideners
- Mixin configuration
- Injection classification
- Compatibility reports

## M8 — server integration pilots

- Use generated reports and tests to implement selected Rust-native server subsystems
- Start with protocol data, NBT, positions, chunks, and registries

---

# 22. Immediate Codex assignment

Implement **M1 Bytecode Inventory** without attempting CFG or Rust code generation yet.

## Required work

1. Inspect the current repository and preserve existing CLI behavior.
2. Add models for method bytecode reports, features, references, and classification.
3. Parse method `Code` attributes using the existing class-file library where possible.
4. Decode JVM instructions safely.
5. Count instructions and opcodes.
6. Extract referenced methods, fields, and types.
7. Detect at minimum:
   - native methods
   - synchronized methods
   - `invokedynamic`
   - `monitorenter`
   - `monitorexit`
   - reflection API references
   - class-loader references
   - `Unsafe` references
   - native library loading
   - switch instructions
   - exception handlers
8. Add deterministic Green/Yellow/Red classification with reasons.
9. Extend JSON output in a versioned and backward-conscious manner.
10. Add Java fixtures that exercise each detected feature.
11. Add unit and integration tests.
12. Update README and roadmap.

## Suggested CLI behavior

Either extend `inspect` with an opt-in flag:

```bash
ferrum inspect sample.jar --bytecode -o report.json
```

or add a separate command:

```bash
ferrum bytecode sample.jar -o report.json
```

Prefer a separate command if the implementation would make the default inventory substantially slower or the report much larger.

## Acceptance criteria

- Existing M0 tests continue to pass.
- A simple arithmetic method is classified Green.
- A method containing a switch is at least Yellow.
- A method using synchronization is at least Yellow.
- A native method is Red.
- Reflection and `Unsafe` references are Red or Yellow according to documented policy.
- One malformed class does not stop analysis of valid classes.
- Output ordering is deterministic.
- No warnings under Clippy with `-D warnings`.
- Public model types are documented.
- No CFG, SSA, decompiler, or Rust body generation is implemented in this milestone.

---

# 23. Codex implementation constraints

- Do not rewrite the repository from scratch.
- Do not remove fault-tolerant archive behavior.
- Do not couple generic bytecode analysis to Minecraft package names.
- Detection rules should be configurable or centralized, not scattered.
- Prefer explicit enums over unstructured strings for machine-readable features.
- Reasons may be human-readable strings, but classification must be an enum.
- Preserve source identifiers in all reports.
- Keep report serialization stable and deterministic.
- Add comments only where they explain invariants or non-obvious JVM behavior.
- Do not claim semantic equivalence from bytecode inventory alone.

---

# 24. Proposed first implementation sequence

1. Add descriptor and reference models to `rom-model`.
2. Create `rom-bytecode` or add a temporary internal module to `rom-importer`.
3. Extract `Code` attributes.
4. Implement safe instruction iteration.
5. Implement opcode histogram.
6. Resolve constant-pool method, field, and class references.
7. Implement feature detectors.
8. Implement classification policy.
9. Add report serialization.
10. Add fixtures and tests.
11. Expose through CLI.
12. Update documentation.

Prefer correctness and clean data contracts over premature optimization.

---

# 25. Final project definition

RoM Porting Kit is best understood as:

> A Rust-based JVM and Minecraft porting assistance toolchain that converts JARs into structured knowledge: symbols, bytecode inventories, control-flow graphs, typed IR, migration classifications, Rust skeletons, and differential tests.

Its purpose is not to replace human architecture decisions. Its purpose is to make those decisions faster, safer, measurable, and repeatable.

The desired long-term result is not “Java code mechanically disguised as Rust.”

The desired result is:

> A clean Rust-native Minecraft server whose behavior is validated against the Java reference implementation, with RoM automating the expensive analysis and migration work required to build it.
