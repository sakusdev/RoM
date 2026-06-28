#!/usr/bin/env python3
import json
import os
import socket
import struct
import sys
import time
import uuid
import zlib
from pathlib import Path

HOST = os.environ.get("MINECRAFT_HOST", "127.0.0.1")
PORT = int(os.environ.get("MINECRAFT_PORT", "25566"))
OUTPUT = Path(os.environ.get("REGISTRY_MANIFEST_OUTPUT", "registry-manifest-26.1.2.json"))
PROTOCOL = 775
USERNAME = "FerrumRegistryProbe"


def write_varint(value: int) -> bytes:
    value &= 0xFFFFFFFF
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            byte |= 0x80
        out.append(byte)
        if not value:
            return bytes(out)


def read_varint(data: bytes, offset: int = 0) -> tuple[int, int]:
    value = 0
    for position in range(5):
        if offset >= len(data):
            raise ValueError("truncated VarInt")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << (7 * position)
        if byte & 0x80 == 0:
            if value & (1 << 31):
                value -= 1 << 32
            return value, offset
    raise ValueError("VarInt exceeds five bytes")


def read_varint_socket(sock: socket.socket) -> int:
    value = 0
    for position in range(5):
        byte = recv_exact(sock, 1)[0]
        value |= (byte & 0x7F) << (7 * position)
        if byte & 0x80 == 0:
            if value & (1 << 31):
                value -= 1 << 32
            return value
    raise ValueError("VarInt exceeds five bytes")


def write_string(value: str) -> bytes:
    encoded = value.encode("utf-8")
    return write_varint(len(encoded)) + encoded


def read_string(data: bytes, offset: int) -> tuple[str, int]:
    length, offset = read_varint(data, offset)
    if length < 0:
        raise ValueError(f"negative string length {length}")
    end = offset + length
    if end > len(data):
        raise ValueError("truncated string")
    return data[offset:end].decode("utf-8"), end


def recv_exact(sock: socket.socket, length: int) -> bytes:
    chunks = bytearray()
    while len(chunks) < length:
        chunk = sock.recv(length - len(chunks))
        if not chunk:
            raise EOFError("socket closed")
        chunks.extend(chunk)
    return bytes(chunks)


def frame_packet(packet_id: int, payload: bytes, compression_threshold: int | None) -> bytes:
    packet = write_varint(packet_id) + payload
    if compression_threshold is None:
        framed = packet
    elif len(packet) >= compression_threshold:
        framed = write_varint(len(packet)) + zlib.compress(packet)
    else:
        framed = write_varint(0) + packet
    return write_varint(len(framed)) + framed


def read_packet(sock: socket.socket, compression_threshold: int | None) -> tuple[int, bytes]:
    frame_length = read_varint_socket(sock)
    if frame_length < 0 or frame_length > 16 * 1024 * 1024:
        raise ValueError(f"invalid frame length {frame_length}")
    frame = recv_exact(sock, frame_length)
    if compression_threshold is not None:
        data_length, offset = read_varint(frame, 0)
        compressed_or_plain = frame[offset:]
        if data_length == 0:
            packet = compressed_or_plain
        else:
            packet = zlib.decompress(compressed_or_plain)
            if len(packet) != data_length:
                raise ValueError(
                    f"decompressed packet length mismatch {len(packet)} != {data_length}"
                )
    else:
        packet = frame
    packet_id, offset = read_varint(packet, 0)
    return packet_id, packet[offset:]


def skip_nbt_payload(data: bytes, offset: int, tag_type: int) -> int:
    if tag_type == 0:
        return offset
    if tag_type == 1:
        return offset + 1
    if tag_type == 2:
        return offset + 2
    if tag_type in (3, 5):
        return offset + 4
    if tag_type in (4, 6):
        return offset + 8
    if tag_type == 7:
        length = struct.unpack_from(">i", data, offset)[0]
        if length < 0:
            raise ValueError("negative NBT byte-array length")
        return offset + 4 + length
    if tag_type == 8:
        length = struct.unpack_from(">H", data, offset)[0]
        return offset + 2 + length
    if tag_type == 9:
        child_type = data[offset]
        length = struct.unpack_from(">i", data, offset + 1)[0]
        if length < 0:
            raise ValueError("negative NBT list length")
        cursor = offset + 5
        for _ in range(length):
            cursor = skip_nbt_payload(data, cursor, child_type)
        return cursor
    if tag_type == 10:
        cursor = offset
        while True:
            child_type = data[cursor]
            cursor += 1
            if child_type == 0:
                return cursor
            name_length = struct.unpack_from(">H", data, cursor)[0]
            cursor += 2 + name_length
            cursor = skip_nbt_payload(data, cursor, child_type)
    if tag_type == 11:
        length = struct.unpack_from(">i", data, offset)[0]
        if length < 0:
            raise ValueError("negative NBT int-array length")
        return offset + 4 + length * 4
    if tag_type == 12:
        length = struct.unpack_from(">i", data, offset)[0]
        if length < 0:
            raise ValueError("negative NBT long-array length")
        return offset + 4 + length * 8
    raise ValueError(f"unsupported NBT tag type {tag_type}")


def skip_network_nbt(data: bytes, offset: int) -> int:
    if offset >= len(data):
        raise ValueError("truncated network NBT")
    tag_type = data[offset]
    return skip_nbt_payload(data, offset + 1, tag_type)


def parse_known_packs(payload: bytes) -> list[dict[str, str]]:
    count, offset = read_varint(payload, 0)
    packs = []
    for _ in range(count):
        namespace, offset = read_string(payload, offset)
        pack_id, offset = read_string(payload, offset)
        version, offset = read_string(payload, offset)
        packs.append({"namespace": namespace, "id": pack_id, "version": version})
    if offset != len(payload):
        raise ValueError("known-packs packet has trailing bytes")
    return packs


def encode_known_packs(packs: list[dict[str, str]]) -> bytes:
    out = bytearray(write_varint(len(packs)))
    for pack in packs:
        out.extend(write_string(pack["namespace"]))
        out.extend(write_string(pack["id"]))
        out.extend(write_string(pack["version"]))
    return bytes(out)


def parse_feature_flags(payload: bytes) -> list[str]:
    count, offset = read_varint(payload, 0)
    values = []
    for _ in range(count):
        value, offset = read_string(payload, offset)
        values.append(value)
    if offset != len(payload):
        raise ValueError("feature-flags packet has trailing bytes")
    return values


def parse_registry(payload: bytes) -> dict:
    registry_id, offset = read_string(payload, 0)
    count, offset = read_varint(payload, offset)
    entries = []
    data_entries = 0
    for numeric_id in range(count):
        entry_id, offset = read_string(payload, offset)
        if offset >= len(payload):
            raise ValueError(f"truncated registry marker for {registry_id}/{entry_id}")
        has_data = payload[offset]
        offset += 1
        if has_data == 1:
            data_entries += 1
            offset = skip_network_nbt(payload, offset)
        elif has_data != 0:
            raise ValueError(
                f"invalid registry marker {has_data} for {registry_id}/{entry_id}"
            )
        entries.append({"id": entry_id, "numeric_id": numeric_id, "has_data": bool(has_data)})
    if offset != len(payload):
        raise ValueError(f"registry {registry_id} has {len(payload) - offset} trailing bytes")
    return {
        "id": registry_id,
        "entry_count": count,
        "data_entry_count": data_entries,
        "entries": entries,
    }


def parse_tags(payload: bytes) -> list[dict]:
    registry_count, offset = read_varint(payload, 0)
    registries = []
    for _ in range(registry_count):
        registry_id, offset = read_string(payload, offset)
        tag_count, offset = read_varint(payload, offset)
        tags = []
        for _ in range(tag_count):
            tag_id, offset = read_string(payload, offset)
            entry_count, offset = read_varint(payload, offset)
            entries = []
            for _ in range(entry_count):
                entry, offset = read_varint(payload, offset)
                entries.append(entry)
            tags.append({"id": tag_id, "entries": entries})
        registries.append({"id": registry_id, "tags": tags})
    if offset != len(payload):
        raise ValueError(f"tags packet has {len(payload) - offset} trailing bytes")
    return registries


def connect_with_retry() -> socket.socket:
    deadline = time.monotonic() + 90
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            return socket.create_connection((HOST, PORT), timeout=5)
        except OSError as error:
            last_error = error
            time.sleep(1)
    raise RuntimeError(f"server did not become reachable: {last_error}")


def main() -> None:
    sock = connect_with_retry()
    sock.settimeout(30)
    compression_threshold: int | None = None

    handshake = (
        write_varint(PROTOCOL)
        + write_string(HOST)
        + struct.pack(">H", PORT)
        + write_varint(2)
    )
    sock.sendall(frame_packet(0x00, handshake, None))
    sock.sendall(
        frame_packet(0x00, write_string(USERNAME) + uuid.uuid4().bytes, None)
    )

    packet_id, payload = read_packet(sock, compression_threshold)
    if packet_id == 0x03:
        compression_threshold, offset = read_varint(payload, 0)
        if offset != len(payload):
            raise ValueError("set-compression packet has trailing bytes")
        packet_id, payload = read_packet(sock, compression_threshold)
    if packet_id == 0x00:
        reason, _ = read_string(payload, 0)
        raise RuntimeError(f"login disconnected: {reason}")
    if packet_id != 0x02:
        raise RuntimeError(f"expected Login Success 0x02, got {packet_id:#x}")

    sock.sendall(frame_packet(0x03, b"", compression_threshold))

    result = {
        "minecraft_version": "26.1.2",
        "protocol_version": PROTOCOL,
        "packet_sequence": [],
        "known_packs": [],
        "feature_flags": [],
        "registries": [],
        "tags": [],
    }

    while True:
        packet_id, payload = read_packet(sock, compression_threshold)
        result["packet_sequence"].append(packet_id)
        if packet_id == 0x0E:
            packs = parse_known_packs(payload)
            result["known_packs"] = packs
            sock.sendall(
                frame_packet(0x07, encode_known_packs(packs), compression_threshold)
            )
        elif packet_id == 0x0C:
            result["feature_flags"] = parse_feature_flags(payload)
        elif packet_id == 0x07:
            result["registries"].append(parse_registry(payload))
        elif packet_id == 0x0D:
            result["tags"] = parse_tags(payload)
        elif packet_id == 0x03:
            sock.sendall(frame_packet(0x03, b"", compression_threshold))
            break

    OUTPUT.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(
        json.dumps(
            {
                "output": str(OUTPUT),
                "registry_count": len(result["registries"]),
                "registry_entries": sum(r["entry_count"] for r in result["registries"]),
                "data_entries": sum(r["data_entry_count"] for r in result["registries"]),
                "tag_registry_count": len(result["tags"]),
                "packet_sequence": result["packet_sequence"],
            },
            indent=2,
        )
    )
    sock.close()


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"registry capture failed: {error}", file=sys.stderr)
        raise
