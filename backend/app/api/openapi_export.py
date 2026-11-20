"""Export the backend OpenAPI schema for frontend consumers (Member 3).

Usage, from ``backend/``::

    uv run python -m app.api.openapi_export            # writes ../docs/api/openapi.json
    uv run python -m app.api.openapi_export OUT.json

A test fails when the committed ``docs/api/openapi.json`` drifts from the live schema.
"""

import json
import sys
from pathlib import Path
from typing import Any

DEFAULT_OUTPUT = Path(__file__).resolve().parents[3] / "docs" / "api" / "openapi.json"


def render_openapi() -> str:
    from app.main import app

    schema: dict[str, Any] = app.openapi()
    return json.dumps(schema, indent=2, sort_keys=True, ensure_ascii=False) + "\n"


def main(argv: list[str]) -> int:
    output = Path(argv[1]) if len(argv) > 1 else DEFAULT_OUTPUT
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(render_openapi(), encoding="utf-8")
    sys.stdout.write(f"OpenAPI schema written to {output.name}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
