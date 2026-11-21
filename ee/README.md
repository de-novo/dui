# dui Enterprise (`ee/`)

This directory contains commercial enterprise features for `dui`, licensed under the [dui Enterprise License](LICENSE) rather than Apache-2.0.

## Open-Core Boundary

- **Open-source core (`Apache-2.0`)**: Everything outside `ee/` directories — including `dui-core`, `crates/dui` (CLI, MCP, local SQLite storage, capture/diff/transitions/comments), `apps/canvas`, and `apps/desktop` (local GPUI desktop viewer).
- **Enterprise (`ee/LICENSE`)**: Commercial team and organization capabilities placed inside `ee/` (such as audit logs, SSO/SAML/OIDC, RBAC, remote team sync, and organization-wide coverage dashboards).
- **One-way dependency rule**: Core crates and local apps must never depend on `ee/`. Open-source builds compile and run cleanly with `ee/` excluded.
