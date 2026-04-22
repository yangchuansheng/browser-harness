"""Deprecated compatibility alias for the Rust-native admin shim."""

from legacy_warnings import warn_legacy_surface

warn_legacy_surface(
    "`import admin` is deprecated; use `admin_cli` or the `browser-harness` CLI instead."
)

from admin_cli import *  # noqa: F401,F403
