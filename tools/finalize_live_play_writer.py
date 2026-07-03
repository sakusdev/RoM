from pathlib import Path

paths = [
    Path('.github/workflows/build-live-play-writer.yml'),
    Path('.github/workflows/finalize-live-play-writer.yml'),
    Path('tools/patch_live_writer_helpers.py'),
    Path('tools/patch_live_writer_output_docs.py'),
    Path('tools/patch_live_writer_play.py'),
    Path('tools/patch_live_writer_state.py'),
    Path('tools/patch_live_writer_tests.py'),
    Path('tools/patch_live_writer_thread_local.py'),
    Path('tools/finalize_live_play_writer.py'),
]
for path in paths:
    path.unlink(missing_ok=True)
