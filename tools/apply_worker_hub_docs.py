from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}: {old[:180]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "crates/ferrum-runtime/src/lib.rs",
    "//! small state runner that applies queued inputs in a stable order.\n\nuse std::{\n",
    "//! small state runner that applies queued inputs in a stable order. A bounded\n"
    "//! worker hub provides the non-blocking boundary for future socket workers.\n\n"
    "mod worker;\n\n"
    "pub use worker::{\n"
    "    ConnectionWorker, WorkerBroadcastReport, WorkerConnector, WorkerControlError,\n"
    "    WorkerIngressReport, WorkerInputError, WorkerOutputError, WorkerReceiveError,\n"
    "    WorkerRuntime, worker_channel,\n"
    "};\n\n"
    "use std::{\n",
)

replace_once(
    "README.md",
    "- Deterministic 3×3 chunk views\n",
    "- Bounded configurable chunk views\n",
)
replace_once(
    "README.md",
    "- System chat and Keep Alive validation\n"
    "- Version-neutral 20 TPS scheduling and bounded deterministic input primitives\n",
    "- Configurable system chat and Keep Alive validation\n"
    "- Version-neutral 20 TPS scheduling and bounded deterministic input primitives\n"
    "- Generic bounded worker command channels and independently bounded non-blocking connection outputs\n",
)
replace_once(
    "README.md",
    "→ 3×3 Chunk View\n",
    "→ Configured Bounded Chunk View\n",
)
replace_once(
    "README.md",
    "- Dedicated network-worker to authoritative-world-runtime queues\n",
    "- Live TCP wiring into the bounded worker hub and authoritative 20 TPS owner\n",
)
replace_once(
    "README.md",
    "- Runtime replacement of remaining server-policy and gameplay constants with explicit runtime configuration\n",
    "",
)
replace_once(
    "README.md",
    "- `ferrum-runtime` — fixed-rate ticks, bounded inputs, and deterministic mutation ordering\n",
    "- `ferrum-runtime` — fixed-rate ticks, bounded inputs, deterministic mutation ordering, and bounded worker channels\n",
)
replace_once(
    "README.md",
    "1. Wire dedicated network workers into the authoritative 20 TPS runtime\n",
    "1. Wire the bounded worker hub into live TCP and the authoritative 20 TPS runtime\n",
)

roadmap = "docs/SERVER_ROADMAP.md"
replace_once(
    roadmap,
    "- Globally bounded, per-connection sequenced input queues\n"
    "- Deterministic fair per-tick input draining and mutation budgets\n",
    "- Globally bounded, per-connection sequenced input queues\n"
    "- Deterministic fair per-tick input draining and mutation budgets\n"
    "- A generic bounded worker-command hub for future network reader workers\n"
    "- Independently bounded, non-blocking output queues for each connection writer\n"
    "- Explicit registration, replacement, disconnect cleanup, overload reporting, and slow-output isolation\n",
)
replace_once(
    roadmap,
    "- Add a generic deterministic state runner for applying envelopes at an authoritative tick.\n"
    "- Cover timing, overload, fairness, queue limits, disconnect cleanup, and mutation order with tests.\n",
    "- Add a generic deterministic state runner for applying envelopes at an authoritative tick.\n"
    "- Add a generic socket-independent worker hub that ingests registered connection inputs into the deterministic queue.\n"
    "- Add independently bounded non-blocking output queues so one full or disconnected writer cannot block another connection.\n"
    "- Remove pending authoritative input and output registration when a connection disconnects or is replaced.\n"
    "- Cover timing, overload, fairness, queue limits, disconnect cleanup, mutation order, and slow-output isolation with tests.\n",
)
replace_once(
    roadmap,
    "- Move packet readers into independent network workers that publish bounded input envelopes.\n"
    "- Run one shared 20 TPS world loop instead of per-connection timing.\n"
    "- Route resulting chunk, movement, Keep Alive, and disconnect output back to connection writers.\n"
    "- Ensure a slow reader or writer cannot block unrelated players.\n"
    "- Add integration tests for deterministic ordering across multiple simulated connections.\n",
    "- Wire live packet readers into the bounded worker connector and classify decoded Play inputs.\n"
    "- Run one shared 20 TPS world loop instead of per-connection timing.\n"
    "- Route resulting chunk, movement, Keep Alive, and disconnect output through dedicated connection writers.\n"
    "- Apply explicit overload and slow-client disconnect policy at the live socket boundary.\n"
    "- Add integration tests for deterministic ordering across multiple simulated connections.\n",
)
