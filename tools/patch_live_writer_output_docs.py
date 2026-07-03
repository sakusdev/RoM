from pathlib import Path
p = Path('crates/ferrum-server/src/authoritative_runtime.rs')
s = p.read_text()
old = '''#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayOutput {
    Packet(Vec<u8>),
    Disconnect(String),
}
'''
new = '''#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayOutput {
    /// Unframed protocol packet bytes including the packet ID.
    Packet(Vec<u8>),
    /// Request a protocol-aware Play disconnect with this reason.
    Disconnect(String),
}
'''
if old not in s:
    raise SystemExit('PlayOutput marker not found')
p.write_text(s.replace(old, new, 1))
