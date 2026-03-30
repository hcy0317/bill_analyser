from __future__ import annotations

import os
from pathlib import Path
import tempfile


def write_text_atomic(path: Path, content: str, *, encoding: str = "utf-8") -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    file_descriptor, raw_temp_path = tempfile.mkstemp(
        dir=path.parent,
        prefix=f".{path.name}.",
        suffix=".tmp",
        text=True,
    )
    temp_path = Path(raw_temp_path)

    try:
        with os.fdopen(file_descriptor, "w", encoding=encoding) as handle:
            handle.write(content)
        temp_path.replace(path)
    except Exception:
        try:
            temp_path.unlink(missing_ok=True)
        except OSError:
            pass
        raise
