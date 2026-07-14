#!/usr/bin/env python3
from pathlib import Path

path = Path("crates/ferrum-server/src/admin_gui.rs")
text = path.read_text()
replacements = {
    "if path.contains(['\\r', '\\n', '\\0']) {": "if path.bytes().any(|byte| matches!(byte, b'\\r' | b'\\n' | b'\\0')) {",
    "if line.starts_with([' ', '\\t']) {": "if line.starts_with(' ') || line.starts_with('\\t') {",
    "if value.contains(['\\r', '\\n', '\\0']) {": "if value.bytes().any(|byte| matches!(byte, b'\\r' | b'\\n' | b'\\0')) {",
    'let method = request_parts.next().context("HTTP method is missing")?;': 'let method = request_parts\\n        .next()\\n        .context("HTTP method is missing")?\\n        .to_owned();',
    'let path = request_parts.next().context("HTTP path is missing")?;': 'let path = request_parts\\n        .next()\\n        .context("HTTP path is missing")?\\n        .to_owned();',
    'if !matches!(method, "GET" | "POST") {': 'if !matches!(method.as_str(), "GET" | "POST") {',
    '        method: method.to_owned(),\\n        path: path.to_owned(),': '        method,\\n        path,',
}
for old, new in replacements.items():
    if new in text:
        continue
    if old not in text:
        raise SystemExit(f"missing admin GUI parser anchor: {old}")
    text = text.replace(old, new, 1)
path.write_text(text)
