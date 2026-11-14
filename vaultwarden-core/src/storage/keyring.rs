use keyring::Entry;
use rand::RngCore;
use crate::storage::error::StorageError;

const SERVICE: &str = "vaultwarden-desktop";
const KEY_USER: &str = "app_master_key";

/// Returns the 32-byte master key from OS keyring, creating it only if no
/// entry has ever been set.
///
/// This distinction matters: `get_password()` can fail for reasons that
/// have nothing to do with "no key exists yet" - `NoStorageAccess` (the
/// keyring is locked, no Secret Service session unlocked), `PlatformFailure`
/// (D-Bus hiccup), etc. Treating every failure as "first launch, generate
/// a fresh key" silently replaces the real master key with a new random
/// one whenever the keyring is merely unavailable for a moment - and every
/// `accounts.enc` written with the OLD key then permanently fails to
/// decrypt with "data tampered or wrong key", even though nothing was
/// actually tampered. Only `keyring::Error::NoEntry` means "never set" and
/// is safe to treat as first-launch; every other error must propagate.
pub fn get_or_create_master_key() -> Result<[u8; 32], StorageError> {
    let entry = Entry::new(SERVICE, KEY_USER)
        .map_err(|e| StorageError::Keyring(e.to_string()))?;

    match entry.get_password() {
        Ok(hex_key) => {
            let mut key = [0u8; 32];
            // A stored value that isn't exactly 64 valid hex chars means
            // the entry is corrupted (e.g. a leftover value from a prior
            // backend/bug, or manual tampering) - this must be a clear,
            // recoverable error, never a panic. The caller (main binary)
            // decides what to do; silently regenerating here would be the
            // exact same "undecryptable accounts.enc" bug this function
            // already guards against for other keyring error types.
            hex::decode_to_slice(hex_key.trim(), &mut key).map_err(|e| {
                StorageError::Keyring(format!(
                    "OS keyring entry for the master key is corrupted (not valid 64-char hex: {e}). \
                     Delete the '{SERVICE}'/'{KEY_USER}' keyring entry to let the app generate a \
                     fresh one — note this makes any existing accounts.enc permanently undecryptable, \
                     so you will need to re-add your accounts afterward."
                ))
            })?;
            Ok(key)
        }
        Err(keyring::Error::NoEntry) => {
            let mut key = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut key);
            entry
                .set_password(&hex::encode(key))
                .map_err(|e| StorageError::Keyring(e.to_string()))?;
            Ok(key)
        }
        Err(e) => Err(StorageError::Keyring(format!(
            "OS keyring unavailable ({e}) — refusing to generate a new master key, \
             which would make existing encrypted accounts permanently undecryptable. \
             Ensure your OS keyring/Secret Service is unlocked and try again."
        ))),
    }
}

/// Delete the master key from the OS keyring (used on account reset).
pub fn delete_master_key() -> Result<(), StorageError> {
    let entry = Entry::new(SERVICE, KEY_USER)
        .map_err(|e| StorageError::Keyring(e.to_string()))?;
    entry
        .delete_credential()
        .map_err(|e| StorageError::Keyring(e.to_string()))
}
