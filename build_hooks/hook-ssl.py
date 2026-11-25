"""Preserve optional trust-policy data used by the build interpreter's SSL module."""

from pathlib import Path
import ssl

# Some managed Python distributions reference this file relative to ssl.__file__.
# Stock Python has no such resource. Keep the existing trust policy intact in either case.
trust_policy = Path(ssl.__file__).with_name("codex-rejected-cas.pem")
datas = [(str(trust_policy), ".")] if trust_policy.is_file() else []
