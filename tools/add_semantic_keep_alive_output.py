from pathlib import Path

path = Path('crates/ferrum-server/src/authoritative_runtime.rs')
source = path.read_text()
old = '''pub enum PlayOutput {
    /// Unframed protocol packet bytes including the packet ID.
    Packet(Vec<u8>),
    /// Request a protocol-aware Play disconnect with this reason.
    Disconnect(String),
}
'''
new = '''pub enum PlayOutput {
    /// Unframed protocol packet bytes including the packet ID.
    Packet(Vec<u8>),
    /// Request a protocol-aware Keep Alive packet with this identifier.
    KeepAliveRequest(i64),
    /// Request a protocol-aware Play disconnect with this reason.
    Disconnect(String),
}
'''
if old not in source:
    raise SystemExit('PlayOutput marker not found')
path.write_text(source.replace(old, new, 1))
