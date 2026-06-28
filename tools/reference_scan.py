#!/usr/bin/env python3
"""Tiny dependency-free reference scanner used to validate test fixtures.

It intentionally parses only enough of JVMS chapter 4 to list class/member names.
The production implementation lives in Rust.
"""
from __future__ import annotations

import argparse
import json
import struct
import zipfile
from dataclasses import dataclass
from pathlib import Path


class Reader:
    def __init__(self, data: bytes):
        self.data = data
        self.pos = 0

    def u1(self) -> int:
        value = self.data[self.pos]
        self.pos += 1
        return value

    def u2(self) -> int:
        value = struct.unpack_from(">H", self.data, self.pos)[0]
        self.pos += 2
        return value

    def u4(self) -> int:
        value = struct.unpack_from(">I", self.data, self.pos)[0]
        self.pos += 4
        return value

    def take(self, size: int) -> bytes:
        value = self.data[self.pos : self.pos + size]
        self.pos += size
        return value


def skip_attributes(r: Reader) -> int:
    count = r.u2()
    for _ in range(count):
        r.u2()
        r.take(r.u4())
    return count


def parse_class(data: bytes) -> dict:
    r = Reader(data)
    if r.u4() != 0xCAFEBABE:
        raise ValueError("bad class magic")
    minor, major = r.u2(), r.u2()
    cp_count = r.u2()
    cp: list[object | None] = [None] * cp_count
    i = 1
    while i < cp_count:
        tag = r.u1()
        if tag == 1:
            cp[i] = ("utf8", r.take(r.u2()).decode("utf-8", "replace"))
        elif tag in (3, 4):
            r.take(4)
        elif tag in (5, 6):
            r.take(8)
            i += 1
        elif tag == 7:
            cp[i] = ("class", r.u2())
        elif tag == 8:
            r.take(2)
        elif tag in (9, 10, 11, 12, 17, 18):
            r.take(4)
        elif tag == 15:
            r.take(3)
        elif tag in (16, 19, 20):
            r.take(2)
        else:
            raise ValueError(f"unknown constant pool tag {tag}")
        i += 1

    def utf8(index: int) -> str:
        item = cp[index]
        if not item or item[0] != "utf8":
            raise ValueError(f"constant #{index} is not utf8")
        return item[1]

    def class_name(index: int) -> str:
        item = cp[index]
        if not item or item[0] != "class":
            raise ValueError(f"constant #{index} is not class")
        return utf8(item[1])

    access = r.u2()
    this_class, super_class = r.u2(), r.u2()
    interfaces = [class_name(r.u2()) for _ in range(r.u2())]

    fields = []
    for _ in range(r.u2()):
        flags, name, descriptor = r.u2(), utf8(r.u2()), utf8(r.u2())
        attributes_count = skip_attributes(r)
        fields.append({"name": name, "descriptor": descriptor, "access_bits": flags, "attributes_count": attributes_count})

    methods = []
    for _ in range(r.u2()):
        flags, name, descriptor = r.u2(), utf8(r.u2()), utf8(r.u2())
        attributes_count = skip_attributes(r)
        methods.append({"name": name, "descriptor": descriptor, "access_bits": flags, "attributes_count": attributes_count})

    attributes_count = skip_attributes(r)
    internal = class_name(this_class)
    return {
        "internal_name": internal,
        "dotted_name": internal.replace("/", "."),
        "super_name": class_name(super_class) if super_class else None,
        "interfaces": interfaces,
        "major": major,
        "minor": minor,
        "access_bits": access,
        "constant_pool_entries": cp_count - 1,
        "attributes_count": attributes_count,
        "fields": fields,
        "methods": methods,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("jar", type=Path)
    args = parser.parse_args()
    classes = []
    with zipfile.ZipFile(args.jar) as zf:
        for name in sorted(zf.namelist()):
            if name.endswith(".class") and name != "module-info.class":
                item = parse_class(zf.read(name))
                item["archive_path"] = name
                classes.append(item)
    print(json.dumps(classes, indent=2))


if __name__ == "__main__":
    main()
