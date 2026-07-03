from pathlib import Path

for path in (
    Path('.github/workflows/build-semantic-keep-alive-output.yml'),
    Path('.github/workflows/finalize-semantic-keep-alive-output.yml'),
    Path('tools/finalize_semantic_keep_alive.py'),
):
    path.unlink(missing_ok=True)
