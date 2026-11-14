# Vaultwarden/Bitwarden Desktop Specification

## 1. Overview & Objectives

A lightweight, cross-platform, cross-architecture desktop client for
**Vaultwarden** (self-hosted) and **official Bitwarden cloud**
(`vault.bitwarden.com` / `vault.bitwarden.eu`), built with **Tauri v2** and
**Rust**. Forked from `passbolt-desktop` — the Tauri shell, SQLite/FTS5
cache pattern, and Svelte 5 UI skeleton carry over; the crypto, auth, and
sync engine are a ground-up rewrite because Bitwarden's trust model has
nothing in common with Passbolt's OpenPGP model (see §9 for what changed
and why).

### Primary Goals
- **Cross-Platform & Cross-Arch:** Linux, macOS, Windows, `x86_64` + `aarch64`.
- **One client, two server families:** Self-hosted Vaultwarden and official
  Bitwarden cloud (US + EU regions) via the same API client, server URL is
  just configuration — Vaultwarden deliberately mirrors the Bitwarden
  client API (`/identity/connect/token`, `/api/sync`), confirmed against
  real Vaultwarden request logs.
- **Full personal + organization vault:** logins, cards, identities, secure
  notes, custom fields, attachments, folders, organizations, and
  collections. No artificial v1 item-type restriction.
- **Offline Cache:** Read-only offline vault browsing from locally cached
  encrypted blobs.
- **Multi-Account Support:** Switch across multiple accounts/servers with
  strict data isolation, same pattern as the Passbolt fork's profile system.
- **Minimal Surface & High Security:** Zero unencrypted secrets on disk, OS
  keyring-backed local storage key, in-memory zeroization of decrypted key
  material and plaintext buffers.
- **Strictly Scoped:** No admin console (member invites, SSO/SCIM config,
  billing, policies editing) — organization *vault* access and collection
  browsing are in scope, organization *administration* is not.

---

## 2. Why the Crypto Engine Is a Full Rewrite, Not a Port

Passbolt's security model is OpenPGP end to end: GPGAuth challenge-response
login keyed off a PGP keypair, resources encrypted directly to every
recipient's PGP public key, v5 metadata itself PGP-encrypted. Bitwarden's
model shares none of that:

| | Passbolt | Bitwarden/Vaultwarden |
|---|---|---|
| Identity | PGP keypair (keyid + passphrase) | Email + master password |
| Login | GPGAuth dual-stage challenge/response | OAuth2 password grant (`/identity/connect/token`) |
| Key derivation | None (key already exists, just passphrase-unlocked) | PBKDF2-SHA256 or Argon2id over master password, salted with email |
| Item encryption | Secret encrypted directly to each recipient's PGP public key (multi-recipient re-encrypt on share) | Item encrypted once with a symmetric item/user key; sharing wraps *that key*, not the item, to the collection/org key |
| Sharing primitive | PGP public key encryption | RSA-OAEP wrapping a symmetric key |
| Sync model | REST-per-resource, timestamp delta (`filter[modified-after]`) | One `/api/sync` document: profile + ciphers + folders + collections + orgs together, revision-date based |
| Wire format for secrets | Armored PGP message | `EncString`: type-prefixed string, e.g. `2.<iv>|<ciphertext>|<mac>` for AES-256-CBC-HMAC-SHA256 |

Net effect: `passbolt-core`'s `crypto/`, `api/`, `sync/`, and `write/`
modules are rewritten, not adapted. The Tauri shell (`state.rs`, `tray.rs`,
single-instance, keyring-backed master key, AES-256-GCM local container
pattern), the SQLite/FTS5 per-profile cache pattern, and the UI shell
(3-column layout, folder tree, drag-and-drop, context menus, clipboard
auto-clear) transfer with only type/IPC-contract changes, not architectural
changes.

---

## 3. Security Architecture & Threat Model

### 3.1 Threat Model & Mitigations
| Threat Vector | Mitigation |
|---|---|
| **Stolen laptop / Disk inspection** | Vault items remain in Bitwarden's native `EncString` ciphertext in SQLite — never decrypted to disk. Account metadata (server URL, email, protected keys) encrypted via AES-256-GCM using a key stored exclusively in the OS Keyring, same container pattern as the Passbolt fork's `profiles.enc`. |
| **Windows Keyring 2.5KB quota** | OS Keyring stores only a single 32-byte local master key. All account records live in an AES-256-GCM encrypted container (`accounts.enc`) on disk. |
| **RAM inspection / Cold boot** | Master Key, Stretched Master Key, Symmetric (User) Key, org keys, and decrypted plaintext are wrapped in `zeroize::ZeroizeOnDrop` buffers, scrubbed after use. |
| **Clipboard snoopers** | Rust writes clipboard directly via `arboard`; plaintext never crosses the Tauri IPC webview bridge on copy; auto-cleared after 30s. |
| **Malicious Webview injection** | Tauri IPC isolation, strongly-typed `#[tauri::command]`s, zero filesystem/shell access from the webview. |
| **Server-side iteration downgrade / weak KDF config** | Client reads KDF type + iterations/memory/parallelism from the server's prelogin response (`/identity/accounts/prelogin`) — never assumes a default silently; warns in UI if an account's KDF config is below current OWASP-recommended minimums. |

### 3.2 Storage Hierarchy
```
~/.config/vaultwarden-desktop/ (or %APPDATA%\vaultwarden-desktop on Windows)
├── accounts.enc                # AES-256-GCM encrypted account list & local settings
└── accounts/
    ├── <account-uuid-1>/
    │   ├── vault.db             # SQLite: ciphers, folders, collections, orgs, attachments metadata
    │   └── attachments/         # Encrypted attachment blobs, as received — never decrypted to disk at rest
    └── <account-uuid-2>/
        └── vault.db
```

### 3.3 Local Master Key Lifecycle
Same keyring pattern as the Passbolt fork: `keyring-rs`, service
`"vaultwarden-desktop"`, user `"app_master_key"`, 32 random bytes on first
run, decrypts `accounts.enc` via AES-256-GCM
(`[12-byte nonce][ciphertext + 16-byte auth tag]`). This key never touches
Bitwarden's own crypto — it only protects the locally-stored account list
(server URLs, emails, cached protected keys) at rest, exactly like
`profiles.enc` did for Passbolt.

---

## 4. Cryptography Engine (Bitwarden Crypto, Ground Truth)

Verified against Bitwarden's published Security Whitepaper and KDF docs,
and Vaultwarden's confirmed API compatibility (real request logs showing
`/identity/connect/token` → `/api/sync` against a live Vaultwarden
instance).

### 4.1 Key Derivation
```
prelogin:  GET /identity/accounts/prelogin  { email }
           -> { kdf: 0 (PBKDF2) | 1 (Argon2id), kdfIterations, kdfMemory?, kdfParallelism? }

Master Key = KDF(master_password, salt = lowercase(email), kdf_params)
  - PBKDF2:   PBKDF2-HMAC-SHA256, iterations from prelogin (>= 600_000 enforced as floor
              for *new* local accounts we create config for; existing accounts use
              whatever the server reports, even if lower — we don't silently upgrade
              someone else's server config)
  - Argon2id: memory (KiB), iterations, parallelism from prelogin

Master Password Hash = PBKDF2-HMAC-SHA256(
  password = Master Key, salt = master_password, iterations = 1
) -> sent as `password` in the token request (over TLS, never raw password)

Stretched Master Key = HKDF-Expand-SHA256(Master Key, length=512 bits)
  split into two 256-bit halves: [enc_key, mac_key]
```
Both PBKDF2 and Argon2id are implemented — real accounts, including many
existing Vaultwarden installs, are still on PBKDF2; Argon2id-only would
lock out logins for a meaningful share of real users.

shortcut: KDF floor enforcement only applies to accounts added through this
app's own "create new account" flow if that's ever built — out of scope
for v1 (login-only, no account creation). Upgrade if account signup is
added later.

### 4.2 `EncString` — Bitwarden's Wire Format
```
type 0: AesCbc256_B64              <iv>|<ciphertext>                    (legacy, decrypt-only)
type 2: AesCbc256_HmacSha256_B64   <iv>|<ciphertext>|<mac>              (current default)
type 4: Rsa2048_OaepSha1_B64       <ciphertext>                         (org/share key wrapping)
```
Format on the wire: `"{type}.{base64(iv)}|{base64(ct)}|{base64(mac)}"`
(mac segment omitted for types without one). Decrypt path for type 2:
split on `.` then `|`, verify HMAC-SHA256(mac_key, iv || ct) against the
mac segment *before* decrypting (encrypt-then-MAC — reject on mismatch,
never decrypt-then-check), then AES-256-CBC decrypt with enc_key.

### 4.3 Symmetric (User) Key Unwrap
```
Account profile (from /api/sync) carries `key`: EncString, the user's
Symmetric Key, encrypted with the Stretched Master Key.

Symmetric Key = decrypt_encstring(profile.key, stretched_master_key)
  -> 512 bits: [item_enc_key (256), item_mac_key (256)]

Every cipher/folder/attachment-key EncString in the vault decrypts with
this Symmetric Key, same encrypt-then-MAC AES-256-CBC-HMAC-SHA256 scheme.
```

### 4.4 Per-Item Keys & Attachments
Each cipher *may* carry its own `key: EncString` (itself encrypted with the
Symmetric Key) — when present, that per-item key, not the Symmetric Key
directly, decrypts the item's `name`/`notes`/`fields`/`login` payloads.
Attachments carry their own `key` the same way, and the attachment blob
itself is AES-256-CBC-HMAC-SHA256 encrypted with that unwrapped key —
decrypt is streamed to a temp buffer, never written to disk in plaintext,
mirroring the "zero unencrypted secrets on disk" rule from the Passbolt
fork.

### 4.5 Organizations & Collections (RSA layer)
```
User keypair: RSA-2048, public key plaintext, private key wrapped as
EncString with the Symmetric Key (from /api/sync profile.privateKey).

Org Symmetric Key: RSA-OAEP-SHA1 encrypted to each member's RSA public key
(type 4 EncString), one entry per member — this is the wrap-the-key
primitive replacing Passbolt's "PGP-encrypt-the-secret-to-every-recipient"
pattern. Unwrap: RSA-OAEP decrypt with own private key -> org's Symmetric
Key -> decrypts that org's ciphers/collections same as personal vault.

Collection membership is an ACL concept (which org members/groups can see
a collection), not a separate encryption boundary — ciphers inside a
collection are encrypted with the org's single Symmetric Key, not a
per-collection key.
```

shortcut: Group-based collection access (org groups, not individual member
ACLs) is read/display-only in v1 — we render what the server already
resolved into visible collections; we don't implement group management
UI. Upgrade only if org admin features are explicitly requested (still a
non-goal, see §9).

---

## 5. Authentication Protocol

```
Desktop Client (Rust)                                Identity Server
       │                                                    │
       ├── GET /identity/accounts/prelogin ─────────────────>│  { email }
       │<── { kdf, kdfIterations, kdfMemory?, kdfParallelism? }
       │
  [ Derive Master Key, Master Password Hash — §4.1 ]
       │
       ├── POST /identity/connect/token ─────────────────────>│
       │   grant_type=password, username=<email>,
       │   password=<master_password_hash>, scope=api offline_access,
       │   client_id=desktop, device_type, device_identifier,
       │   [2FA: two_factor_token, two_factor_provider if challenged]
       │
       │<── 200: { access_token, refresh_token, expires_in, Key, PrivateKey, ... }
       │    OR 400 + { TwoFactorProviders: [...] } -> prompt user, retry with token
       │
       ├── GET /api/sync ───────────────────────────────────>│  Bearer access_token
       │<── { profile, ciphers[], folders[], collections[], organizations[], domains }
```
Access tokens are short-lived; `refresh_token` (stored encrypted in
`accounts.enc`, same treatment as Passbolt's session cookie) re-mints an
access token via `grant_type=refresh_token` on 401, transparently, no
re-prompt unless the refresh token itself is rejected (then fall back to
full re-login prompt).

Two-factor: TOTP (`provider 0`) is the only method guaranteed to work
identically against Vaultwarden and official cloud — WebAuthn/Duo/email
2FA exist on official Bitwarden but have inconsistent Vaultwarden support;
v1 implements TOTP-provider 2FA only, surfaces other providers' challenge
as "use another client to approve" if the server reports them, rather than
pretending to support a flow we can't complete.

---

## 6. Server Targets

One API client, base URLs are configuration, not separate code paths:

| Target | Identity URL | API URL |
|---|---|---|
| Official Bitwarden (US) | `https://identity.bitwarden.com` | `https://api.bitwarden.com` |
| Official Bitwarden (EU) | `https://identity.bitwarden.eu` | `https://api.bitwarden.eu` |
| Vaultwarden (self-hosted) | `<server>/identity` | `<server>/api` |

Account setup flow calls `GET <server>/api/config` first (confirmed real
Vaultwarden/Bitwarden endpoint) to discover the actual identity/api/icons
URLs the server reports, rather than hardcoding path suffixes — this is
what lets one "add account" form handle both official cloud shorthand
(user picks US/EU) and arbitrary self-hosted URLs without a server-type
toggle the user has to get right manually.

---

## 7. Local Database Schema & Cache Engine

Each account maintains an isolated SQLite database (`vault.db`), storing
ciphertext exactly as received — decryption happens in Rust at read time,
never persisted plaintext.

### 7.1 SQLite Schema (core tables)
```sql
CREATE TABLE IF NOT EXISTS ciphers (
    id TEXT PRIMARY KEY,                -- Bitwarden cipher UUID
    organization_id TEXT,               -- NULL = personal vault item
    folder_id TEXT,
    type INTEGER NOT NULL,              -- 1=Login 2=SecureNote 3=Card 4=Identity
    name_enc TEXT NOT NULL,             -- EncString, decrypted client-side for display/search
    notes_enc TEXT,
    login_enc TEXT,                     -- EncString-JSON: username/password/uris/totp
    card_enc TEXT,
    identity_enc TEXT,
    fields_enc TEXT,                    -- EncString-JSON array, custom fields
    key_enc TEXT,                       -- per-item key, EncString, NULL = use account/org Symmetric Key
    favorite INTEGER NOT NULL DEFAULT 0,
    revision_date INTEGER NOT NULL,
    deleted_date INTEGER                -- soft delete (Bitwarden trash), NULL = active
);

CREATE TABLE IF NOT EXISTS folders (
    id TEXT PRIMARY KEY,
    name_enc TEXT NOT NULL,
    revision_date INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS organizations (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,                 -- plaintext, org name is not E2E-encrypted in Bitwarden
    key_enc TEXT NOT NULL,              -- RSA-wrapped org Symmetric Key (type 4 EncString)
    use_totp INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS collections (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    name_enc TEXT NOT NULL,             -- encrypted with that org's Symmetric Key
    read_only INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS cipher_collections (
    cipher_id TEXT NOT NULL,
    collection_id TEXT NOT NULL,
    PRIMARY KEY (cipher_id, collection_id)
);

CREATE TABLE IF NOT EXISTS attachments (
    id TEXT PRIMARY KEY,
    cipher_id TEXT NOT NULL REFERENCES ciphers(id) ON DELETE CASCADE,
    file_name_enc TEXT NOT NULL,
    key_enc TEXT NOT NULL,              -- attachment's own key, EncString
    size INTEGER NOT NULL,
    local_path TEXT                     -- set once downloaded & cached encrypted-at-rest; NULL until fetched
);

-- Decrypted-at-index-time plaintext search cache (name/username/uri only —
-- notes/fields/card/identity are NOT indexed here, same "don't cache more
-- plaintext than needed for search" principle as the Passbolt fork's FTS
-- table).
CREATE VIRTUAL TABLE IF NOT EXISTS ciphers_fts USING fts5(
    name, username, uri,
    content='ciphers', content_rowid='rowid'
);

CREATE TABLE IF NOT EXISTS metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL                 -- last_sync revision date, cached KDF config, etc.
);
```

shortcut: the FTS cache stores decrypted plaintext name/username/uri
on-disk-adjacent in SQLite (same tradeoff the Passbolt fork already made
for searchability) — notes/custom-fields/card/identity numbers are
deliberately excluded from this plaintext index; upgrade only if
full-field search is explicitly requested, since every field added here is
more plaintext at rest.

### 7.2 Sync Engine
Bitwarden's sync is a single full-document pull, not Passbolt's
per-resource delta — there's no `filter[modified-after]` equivalent on
`/api/sync` itself:
```
[ Sync Trigger (App Start / Account Switch / Manual Refresh / Timer) ]
                   │
            Is Network Online?
                   ├─ No ──> [ VaultState::OfflineRead, serve from SQLite ]
                  Yes
                   │
         Access token valid? ── No ──> refresh_token grant ── fails ──> re-login prompt
                   │ Yes
         GET /api/sync (Bearer token)
                   │
   Full document: profile, ciphers[], folders[], collections[],
   organizations[], sends[] (sends deferred, see §9)
                   │
   Transaction: replace ciphers/folders/collections/organizations tables
   wholesale (diffed by id so unrelated FTS rows aren't needlessly
   rebuilt), decrypt name/username/uri into the FTS cache, update
   `metadata.last_sync`
```
No delta/incremental sync exists to optimize against in Bitwarden's API as
confirmed — full-document pull is the correct and only approach, not a
shortcut.

---

## 8. User Interface & Desktop Ergonomics

Reused wholesale from the Passbolt fork's UI shell, rewired to new
types/IPC: `TitleBar`, `Sidebar`, `FolderTree*`, `LockScreen`, `CopyButton`,
`SettingsModal`, context-menu + drag-and-drop patterns, Tailwind design
tokens. New surface needed for item-type-specific fields (Card/Identity
forms, custom fields editor, attachment upload/download UI, org/collection
switcher in the sidebar alongside folders).

### 8.1 Global Keyboard Shortcuts
Same bindings as the Passbolt fork (`Ctrl+Shift+Space` toggle window,
`Ctrl+F` search, `Ctrl+N` new item, `Ctrl+L` lock) — no reason to relearn
muscle memory between the two apps.

### 8.2 Auto-Lock Rules
Unchanged from the Passbolt fork: idle timer (configurable, default 15
min), system suspend/sleep listener, OS session lock listener — all three
zero the Symmetric Key and any unwrapped org keys from memory immediately.

---

## 9. Explicit Non-Goals & Deferrals

- **No organization administration** — member invites, role assignment,
  SSO/SCIM config, policies, billing. Vault access to orgs the user is
  already a member of is in scope; administering the org is not.
- **No Bitwarden Send** — a materially separate feature (its own
  anonymous-link encryption scheme, not tied to vault items). Deferred,
  revisit as its own spec if requested.
- **No Emergency Access** — separate RSA-based trusted-contact feature,
  deferred.
- **No offline writes** — same rule as the Passbolt fork and for the same
  reason: this app is not the place to resolve multi-device edit conflicts
  or re-wrap item keys without a live round-trip to the server's revision
  tracking. All create/update/delete require `VaultState::OnlineReady`.
- **No WebAuthn/Duo/email 2FA** — TOTP-provider 2FA only (§5); other
  providers surface as "approve from another client."
- **No account creation / registration** — this app connects to existing
  Bitwarden/Vaultwarden accounts; signup remains the web vault's job.

---

## 10. Implementation Order

1. **Crypto engine** (`bw-core::crypto`) — KDF (both variants), `EncString`
   encrypt/decrypt, Symmetric Key unwrap, RSA-OAEP org key unwrap. Fully
   unit-testable offline against known Bitwarden test vectors before any
   network code exists — highest-risk piece, do it first and prove it with
   tests, not against a live server.
2. **Auth + API client** (`bw-core::api`) — prelogin, token grant +
   refresh, `/api/config` discovery for server-URL flexibility, TOTP 2FA
   challenge handling.
3. **Sync + local cache** (`bw-core::sync`, `storage::db`) — full-document
   pull, SQLite schema, FTS cache population from decrypted name/username/
   uri.
4. **Read-only UI** — ported 3-column shell, item list/search/detail pane
   for all item types (login/card/identity/note), folder tree, org/
   collection switcher.
5. **CRUD** (`bw-core::write`) — create/update/delete ciphers and folders,
   per-item key generation on create, org/collection key wrapping on
   share — mirrors the Passbolt fork's "online-only, correctness over
   speed" approach to multi-party key handling.
6. **Attachments** — encrypted upload/download, streamed decrypt, no
   plaintext ever written to disk.

Each phase gets its own test pass (`cargo test --workspace`) before moving
to the next, same discipline as the Passbolt fork's implementation order.
