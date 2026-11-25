# NanoDock Versioning & Release Rules

These rules apply to all contributors and AI coding agents working on NanoDock.

## 1. Semantic Versioning (`MAJOR.MINOR.PATCH`)
Follow [Semantic Versioning](https://semver.org/):
- **PATCH (`x.y.Z`)**: Bug fixes, UI/CSS adjustments, i18n/copy fixes, performance tweaks, or internal refactors that do not add user-facing features or break compatibility (e.g., `0.2.0` -> `0.2.1`).
- **MINOR (`x.Y.0`)**: New user-facing features, new UI views/controls, new CLI subcommands, or backward-compatible daemon/API additions. Always reset `PATCH` to `0` (e.g., `0.2.3` -> `0.3.0`).
- **MAJOR (`X.0.0`)**:
  - `0.x.y` -> `1.0.0`: First stable production release.
  - `1.x.y` -> `2.0.0`: Breaking changes (incompatible config schema, CLI flags, or data migration required).
  - **Never** bump `MAJOR` without explicit user approval.

## 2. When to Bump
- Bump the version **once per completed task** (feature, fix batch, or release) right before finalizing your work — do not bump on intermediate edits.
- If a single task combines both a new feature and bug fixes, bump **MINOR** (the higher precedence).
- Skip version bumps for pure documentation-only or rule-only edits unless requested.

## 3. Files That Must Stay in Sync
Every version bump **must** update all 4 manifest files to the exact same version string:
1. `package.json` (`"version"`)
2. `src-tauri/tauri.conf.json` (`"version"`) — *source of truth for the Tauri UI via `getVersion()`*
3. `src-tauri/Cargo.toml` (`version`)
4. `Cargo.toml` (`[workspace.package]` `version`)

## 4. Changelog
- Whenever bumping the version, add an entry at the top of `CHANGELOG.md` using the format:
  ```markdown
  ## [X.Y.Z] - YYYY-MM-DD
  ### Added / Changed / Fixed
  - Concise summary of the change.
  ```
