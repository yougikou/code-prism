#!/usr/bin/env python3
"""Verify that a CodePrism release payload contains working Camel analyzers."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


CAMEL_ANALYZERS = (
    "camel_java_production_metrics.py",
    "camel_java_test_metrics.py",
    "camel_java_production_project_metrics.py",
    "camel_java_test_project_metrics.py",
)

REQUIRED_FILES = (
    *(f"custom_analyzers/{name}" for name in CAMEL_ANALYZERS),
    "custom_analyzers/camel_support/__init__.py",
    "custom_analyzers/camel_support/file_metrics.py",
    "custom_analyzers/camel_support/model.py",
    "templates/codeprism.camel-java-dsl.yaml",
)


def verify(payload_root: Path) -> None:
    missing = [
        relative_path
        for relative_path in REQUIRED_FILES
        if not (payload_root / relative_path).is_file()
    ]
    if missing:
        formatted = "\n".join(f"  - {path}" for path in missing)
        raise RuntimeError(f"release payload is missing required files:\n{formatted}")

    bytecode_artifacts = [
        path.relative_to(payload_root)
        for path in (payload_root / "custom_analyzers").rglob("*")
        if path.name == "__pycache__" or path.suffix in {".pyc", ".pyo"}
    ]
    if bytecode_artifacts:
        formatted = "\n".join(f"  - {path}" for path in bytecode_artifacts)
        raise RuntimeError(
            f"release payload contains platform-specific Python bytecode:\n{formatted}"
        )

    for analyzer_name in CAMEL_ANALYZERS:
        analyzer_path = payload_root / "custom_analyzers" / analyzer_name
        environment = os.environ.copy()
        environment["PYTHONDONTWRITEBYTECODE"] = "1"
        subprocess.run(
            [sys.executable, str(analyzer_path), "test"],
            cwd=payload_root,
            check=True,
            env=environment,
        )


def main() -> int:
    payload_root = Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
    try:
        verify(payload_root)
    except (RuntimeError, subprocess.CalledProcessError) as error:
        print(f"release payload verification failed: {error}", file=sys.stderr)
        return 1
    print("release payload contains all Camel analyzers, support files, template, and tests")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
