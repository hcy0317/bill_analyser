from __future__ import annotations

from pathlib import Path
import os

from scripts.hooks import file_io


def test_write_text_atomic_persists_content(tmp_path: Path) -> None:
    target = tmp_path / "state.json"

    file_io.write_text_atomic(target, '{"ok": true}\n')

    assert target.read_text(encoding="utf-8") == '{"ok": true}\n'


def test_write_text_atomic_cleans_temp_file_on_failure(tmp_path: Path, monkeypatch) -> None:
    target = tmp_path / "state.json"
    original_fdopen = os.fdopen

    def fail_fdopen(*args, **kwargs):
        handle = original_fdopen(*args, **kwargs)
        handle.close()
        raise OSError("fdopen boom")

    monkeypatch.setattr(file_io.os, "fdopen", fail_fdopen)

    try:
        file_io.write_text_atomic(target, "broken")
    except OSError as exc:
        assert str(exc) == "fdopen boom"
    else:
        raise AssertionError("Expected OSError to be raised")

    temp_candidates = list(tmp_path.glob(".state.json.*.tmp"))
    assert temp_candidates == []
