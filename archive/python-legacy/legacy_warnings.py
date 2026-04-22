"""Shared warnings for deprecated Python compatibility surfaces."""

import os
import warnings
from pathlib import Path

SUPPRESS_PY_DEPRECATION_ENV = "BROWSER_HARNESS_SUPPRESS_PY_DEPRECATION"
_TRUE_VALUES = {"1", "true", "yes", "on"}
_ENV_LOADED = False


class LegacyPythonSurfaceWarning(FutureWarning):
    """Visible warning category for deprecated Python compatibility paths."""


def _load_env():
    global _ENV_LOADED
    if _ENV_LOADED:
        return
    path = Path(__file__).parent / ".env"
    if path.exists():
        for line in path.read_text().splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, value = line.split("=", 1)
            os.environ.setdefault(key.strip(), value.strip().strip('"').strip("'"))
    _ENV_LOADED = True


def deprecation_warnings_enabled():
    _load_env()
    return os.environ.get(SUPPRESS_PY_DEPRECATION_ENV, "").strip().lower() not in _TRUE_VALUES


def warn_legacy_surface(message, *, stacklevel=2):
    if not deprecation_warnings_enabled():
        return
    warnings.warn(
        f"{message} Set {SUPPRESS_PY_DEPRECATION_ENV}=1 to suppress this warning.",
        LegacyPythonSurfaceWarning,
        stacklevel=stacklevel,
    )
