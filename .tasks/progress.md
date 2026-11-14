# Progress Ledger — Vaultwarden Desktop

| # | Task | Status | Spec | Quality | Commits | Notes |
|---|------|--------|------|---------|---------|-------|
| 1 | Reset repo & project scaffold rename | DONE | PASS | PASS | - | Clean git init on main branch, icons generated from SVG/PNG |
| 2 | Bitwarden crypto engine (KDF, EncString, Keys) | DONE | PASS | PASS | - | 15/15 unit tests pass (PBKDF2, Argon2id, EncString types 0/2/4, HKDF stretch, TOTP) |
| 3 | Keyring + accounts storage + SQLite/FTS5 | DONE | PASS | PASS | - | accounts.enc AES-256-GCM + vault.db FTS5 search index verified |
| 4 | API client + OAuth2 password grant auth | DONE | PASS | PASS | - | Prelogin, Token, Sync endpoints, server endpoints for Bitwarden US, EU, and Vaultwarden |
| 5 | Full sync engine & SQLite cache | DONE | PASS | PASS | - | Full sync document processing for ciphers, folders, collections, organizations |
| 6 | Tauri IPC command bridge & session state | DONE | PASS | PASS | - | Full e2e test passing (add, switch, unlock, search, reveal, lock) |
| 7 | UI adaptations for Bitwarden | IN PROGRESS | PASS | PASS | - | Build passes (index.html, brand blue scales, LockScreen, AddProfile) |
| 8 | Online CRUD & Attachments | PLANNED | - | - | - | Milestone 9 in roadmap |
| 9 | Desktop polish & packaging | PLANNED | - | - | - | Single-instance, system tray, auto-lock |
