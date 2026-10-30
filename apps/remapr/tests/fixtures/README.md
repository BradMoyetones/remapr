# Settings from earlier Remapr versions

Files as Remapr wrote them, to test that newer versions still read them:

- `config-0.5.toml`: `config.toml` from Remapr 0.1 to 0.5 (format version 1, no `version` line):
  keys as Windows virtual-key codes, modifiers as bits (1 Ctrl, 2 Shift, 4 Alt, 8 Win), the app as
  a program file name. Its last mapping uses `VK_PACKET`, which is no key, and can't be converted.
- `export-0.5.toml`: a file from "Export mappings…" in those versions.

Never edit these: add a file when a format changes.
