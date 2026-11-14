# Vaultwarden Desktop

Vaultwarden Desktop is a lightweight, cross-platform desktop client for [Vaultwarden](https://github.com/dani-garcia/vaultwarden) (self-hosted) and official [Bitwarden](https://bitwarden.com) cloud instances, built with Tauri v2, Rust, Svelte 5, and Tailwind CSS.

It provides a clean, fast desktop user experience with offline-first local caching, instant full-text search, multi-account management, organizations & collections support, full item types (Logins, Cards, Secure Notes, Identities, and custom fields), attachments, and zero plaintext secrets on disk.

---

## Key Features

### Security & Cryptography
- **Zero Plaintext Secrets on Disk:** Sensitive vault data is stored in Bitwarden's native `EncString` format in an isolated local SQLite database per account.
- **Bitwarden-Compatible Crypto:** PBKDF2-SHA256 and Argon2id key derivation, HKDF stretching, AES-256-CBC with HMAC-SHA256 authenticated encryption, and RSA-OAEP for organization key sharing.
- **OS Keyring Integration:** A 32-byte local master key is stored in the system keyring (`SecretService` on Linux, DPAPI on Windows, Keychain on macOS).
- **AES-256-GCM Account Storage:** Account endpoints, tokens, and metadata are stored in an encrypted container (`accounts.enc`) using AES-256-GCM.
- **In-Memory Zeroization:** Decrypted keys and plaintext secret buffers are wrapped in `zeroize::ZeroizeOnDrop` types and erased from memory when dropped or locked.
- **Auto-Lock on Idle & Sleep:** Locks the vault and purges keys from memory after inactivity or system sleep.

### Vault & Credential Management
- **Full Bitwarden Item Types:** Logins, Credit Cards, Secure Notes, and Identities, with custom fields.
- **Integrated TOTP Generator:** Live countdown authenticator codes computed natively.
- **Organizations & Collections:** View and manage personal vault alongside organization vaults and collections.
- **Encrypted Attachments:** Secure file attachment downloading and previewing.
- **Offline Cache & Instant Search:** SQLite FTS5 instant filtering even without internet access.

---

## Architecture Overview

```
vaultwarden-desktop/
├── vaultwarden-core/              # Core Bitwarden/Vaultwarden logic (Agnostic of Tauri/UI)
│   ├── src/
│   │   ├── api/                   # Prelogin, OAuth2 token grant, /api/sync, CRUD
│   │   ├── crypto/                # KDF (PBKDF2/Argon2id), EncString, key unwrap, TOTP
│   │   ├── storage/               # Keyring, accounts.enc, SQLite & FTS5
│   │   └── sync/                  # Full sync engine, offline state machine
│   └── tests/                     # Unit and integration tests
├── src-tauri/                     # Tauri v2 desktop application shell
│   ├── src/
│   │   ├── commands/              # Strongly-typed IPC handlers
│   │   ├── state.rs               # In-memory runtime state and session maps
│   │   ├── tray.rs                # System tray icon and menu setup
│   │   └── lib.rs                 # Plugin registration and lifecycle listeners
│   └── capabilities/              # Tauri v2 security ACL and permissions
└── ui/                            # Frontend interface
    ├── src/
    │   ├── components/            # Svelte 5 components (DetailPane, FolderTree, etc.)
    │   ├── lib/                   # Stores, types, IPC invocation wrappers
    │   └── app.css                # Tailwind CSS v4 design tokens and theme scales
    └── vite.config.ts             # Vite build configuration
```

---

## Development Setup

1. **Prerequisites:**
   - Rust: Stable toolchain (`rustup default stable`)
   - Node.js: Node 18+ and `npm`

2. **Prepare Environment:**
   ```bash
   make dev-setup
   ```

3. **Run Development Build:**
   ```bash
   make run
   ```

---

## Specifications

Detailed architectural design records are preserved in:
- `SPEC.md`: Vaultwarden & Bitwarden Desktop Architecture and Cryptography Specification.
- `TASKS.md`: Milestone checklist and implementation roadmap tracking.
