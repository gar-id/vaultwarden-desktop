# Vaultwarden Desktop — Implementation Roadmap & Status

This document tracks the progressive development milestones of Vaultwarden Desktop (supporting Vaultwarden self-hosted and Bitwarden cloud).

---

## Progress Overview

```
[Milestone 1: Project Skeleton & Workspace Rename] ──── [IN PROGRESS]
       │
       ▼
[Milestone 2: Bitwarden Crypto Engine (KDF & EncString)] ── [PLANNED]
       │
       ▼
[Milestone 3: Keyring & Accounts Storage Layer] ─────────── [PLANNED]
       │
       ▼
[Milestone 4: Bitwarden/Vaultwarden API & Auth] ─────────── [PLANNED]
       │
       ▼
[Milestone 5: Full Sync Engine & SQLite/FTS5 Cache] ─────── [PLANNED]
       │
       ▼
[Milestone 6: Tauri v2 Scaffold & IPC Command Bridge] ───── [PLANNED]
       │
       ▼
[Milestone 7: Frontend UI (Svelte 5 - Logins, Cards, Notes)] [PLANNED]
       │
       ▼
[Milestone 8: Organizations & Collections Support] ──────── [PLANNED]
       │
       ▼
[Milestone 9: Online Vault CRUD & Attachments] ──────────── [PLANNED]
       │
       ▼
[Milestone 10: Desktop Polish, System Tray & Auto-Lock] ─── [PLANNED]
```

---

## Milestone 1: Project Skeleton & Workspace Rename
- [x] Reset Git history to clean initial repository.
- [x] Update Tauri icons to Vaultwarden assets.
- [ ] Rename `passbolt-core` to `vaultwarden-core` in workspace and update Cargo.toml.
- [ ] Update `src-tauri` naming, bundle identifiers, and configs (`com.vaultwarden.desktop`).
- [ ] Update `Makefile` targets and descriptions.
- [ ] Update `ui/package.json` and theme variables.

---

## Milestone 2: Bitwarden Crypto Engine (`vaultwarden-core::crypto`)
- [ ] **2.1 Key Derivation Functions (`crypto::kdf`)**
  - PBKDF2-HMAC-SHA256 (variable iterations, salted by email).
  - Argon2id (iterations, memory, parallelism, salted by email).
  - Master Password Hash computation for OAuth token request.
  - Master Key stretching via HKDF-Expand-SHA256 to 512-bit (enc_key + mac_key).
- [ ] **2.2 Bitwarden `EncString` Format (`crypto::enc_string`)**
  - Type 0: AesCbc256_B64 (`iv|ct`).
  - Type 2: AesCbc256_HmacSha256_B64 (`iv|ct|mac`), encrypt-then-MAC verification.
  - Type 4: Rsa2048_OaepSha1_B64 (`ct`).
- [ ] **2.3 Symmetric Key Management & RSA Keypair (`crypto::keys`)**
  - User Symmetric Key unwrap from `profile.key`.
  - RSA-2048 private key unwrap from `profile.privateKey`.
  - Organization Symmetric Key unwrap using RSA-OAEP.
  - In-memory `zeroize::ZeroizeOnDrop` wrappers for keys and plaintexts.

---

## Milestone 3: Keyring & Accounts Storage Layer (`vaultwarden-core::storage`)
- [ ] **3.1 OS Keyring Integration (`storage::keyring`)**
  - Service `"vaultwarden-desktop"`, user `"app_master_key"`.
  - 32-byte secure random master key generation and retrieval.
- [ ] **3.2 Encrypted Accounts Container (`storage::accounts`)**
  - `accounts.enc` storage with AES-256-GCM.
  - Multi-account management (Vaultwarden self-hosted + Bitwarden US/EU cloud).
- [ ] **3.3 SQLite Vault Database (`storage::db`)**
  - Per-account database (`accounts/<uuid>/vault.db`).
  - Tables: `ciphers`, `folders`, `organizations`, `collections`, `cipher_collections`, `attachments`, `metadata`.
  - FTS5 virtual table for instant search across decrypted name, username, uri.

---

## Milestone 4: Bitwarden/Vaultwarden API & Auth (`vaultwarden-core::api`)
- [ ] **4.1 Config & Prelogin**
  - Server configuration auto-discovery via `GET /api/config`.
  - Email-based KDF parameter discovery via `GET /identity/accounts/prelogin`.
- [ ] **4.2 Authentication Flow**
  - OAuth2 password grant via `POST /identity/connect/token`.
  - TOTP 2FA challenge handling.
  - Token refresh via `grant_type=refresh_token`.
  - Unified client routing for self-hosted and official cloud.

---

## Milestone 5: Full Sync Engine (`vaultwarden-core::sync`)
- [ ] Single-call vault sync via `GET /api/sync`.
- [ ] Atomic database transaction updating ciphers, folders, collections, orgs.
- [ ] FTS5 index update on sync.
- [ ] Offline fallback state machine (`OnlineReady`, `OfflineRead`, `Syncing`, `Locked`).

---

## Milestone 6: Tauri v2 Scaffold & IPC Bridge (`src-tauri`)
- [ ] App state management: session map, in-memory keys, vault connection pool.
- [ ] Account commands: `list_accounts`, `add_account`, `remove_account`, `switch_account`.
- [ ] Vault commands: `unlock_vault`, `lock_vault`, `get_vault_state`, `sync_vault`, `search_ciphers`.
- [ ] Cipher commands: `reveal_cipher`, `copy_cipher_field`, `get_totp_code`.
- [ ] Clipboard auto-clear after 30 seconds.

---

## Milestone 7: Frontend UI (`ui`)
- [ ] Svelte 5 + Tailwind CSS v4 layout adapted for Bitwarden data model.
- [ ] Sidebar: Vault switcher (All, Personal, Organizations), Folder tree, Collections list.
- [ ] Cipher list: Login, Card, Secure Note, Identity filters and search.
- [ ] Detail Pane: full field display (custom fields, hidden fields, TOTP countdown timer).
- [ ] Add/Edit cipher modals with password generator integration.

---

## Milestone 8: Attachments & Polish
- [ ] Encrypted attachment download & view without writing plaintext to disk.
- [ ] System tray icon, window minimize to tray, auto-lock on idle and suspend.
- [ ] Global shortcuts (`Ctrl+Shift+Space`, `Ctrl+F`, `Ctrl+N`, `Ctrl+L`).
