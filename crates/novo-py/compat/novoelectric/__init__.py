"""The old name of the `kopercad` module. Everything is re-exported from it."""

import warnings

import kopercad as _kopercad
from kopercad import *  # noqa: F401,F403
from kopercad import KoperError, NovoError, Project, __version__, symbols  # noqa: F401

warnings.warn(
    "the 'novoelectric' module is now called 'kopercad'; use `import kopercad`",
    DeprecationWarning,
    stacklevel=2,
)

__all__ = ["KoperError", "NovoError", "Project", "__version__", "symbols"]
