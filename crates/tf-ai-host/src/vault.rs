//! `CredentialVault`: the OpenRouter API key in the OS keychain (Windows Credential Manager),
//! through keyring-core. The key can be stored, checked and deleted from outside this
//! crate; only the egress host inside the crate can read it.

use keyring_core::{CredentialStore, Entry, Error as KeyringError};

/// Keychain service of the production entry.
pub const PRODUCTION_SERVICE: &str = "com.typefaced.desktop";
/// Keychain account of the production entry.
pub const PRODUCTION_ACCOUNT: &str = "openrouter-api-key";

/// Longest accepted key, in bytes. OpenRouter keys are well under 100 bytes.
const MAX_KEY_BYTES: usize = 512;

/// Why a vault operation failed. No variant carries the key or keychain secret data.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VaultError {
    #[error("the API key must be 1 to {MAX_KEY_BYTES} visible ASCII characters without spaces")]
    InvalidKey,
    #[error("no API key is stored")]
    NoKey,
    #[error("the OS credential store failed: {0}")]
    Store(String),
    #[error("this platform has no supported OS credential store")]
    Unsupported,
}

impl From<KeyringError> for VaultError {
    fn from(error: KeyringError) -> Self {
        match error {
            KeyringError::NoEntry => Self::NoKey,
            // Display only: the `Debug` form of some variants holds the secret bytes.
            other => Self::Store(other.to_string()),
        }
    }
}

/// One keychain entry that holds the API key.
#[derive(Debug)]
pub struct CredentialVault {
    entry: Entry,
}

impl CredentialVault {
    /// Opens the entry `service` / `account` in `store`. Nothing is written yet.
    pub fn open(store: &CredentialStore, service: &str, account: &str) -> Result<Self, VaultError> {
        Ok(Self {
            entry: store.build(service, account, None)?,
        })
    }

    /// The production entry in Windows Credential Manager.
    pub fn production() -> Result<Self, VaultError> {
        Self::os(PRODUCTION_SERVICE, PRODUCTION_ACCOUNT)
    }

    /// An entry in Windows Credential Manager with `Local` persistence: the key stays
    /// on this machine and never roams with an enterprise profile.
    #[cfg(windows)]
    pub fn os(service: &str, account: &str) -> Result<Self, VaultError> {
        use keyring_core::api::CredentialStoreApi;

        let store = windows_native_keyring_store::Store::new()?;
        let modifiers = std::collections::HashMap::from([("persistence", "Local")]);
        Ok(Self {
            entry: store.build(service, account, Some(&modifiers))?,
        })
    }

    /// No OS store is wired up outside Windows yet.
    #[cfg(not(windows))]
    pub fn os(_service: &str, _account: &str) -> Result<Self, VaultError> {
        Err(VaultError::Unsupported)
    }

    /// Stores `key`, replacing any earlier key.
    pub fn set_key(&self, key: &str) -> Result<(), VaultError> {
        let valid =
            (1..=MAX_KEY_BYTES).contains(&key.len()) && key.bytes().all(|b| b.is_ascii_graphic());
        if !valid {
            return Err(VaultError::InvalidKey);
        }
        Ok(self.entry.set_password(key)?)
    }

    /// Whether a key is stored.
    pub fn has_key(&self) -> Result<bool, VaultError> {
        match self.key() {
            Ok(_) => Ok(true),
            Err(VaultError::NoKey) => Ok(false),
            Err(other) => Err(other),
        }
    }

    /// Deletes the key. Deleting when no key is stored is not an error.
    pub fn delete_key(&self) -> Result<(), VaultError> {
        match self.entry.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(other) => Err(other.into()),
        }
    }

    /// The stored key. Crate-private: no caller outside the egress host can read it.
    pub(crate) fn key(&self) -> Result<String, VaultError> {
        Ok(self.entry.get_password()?)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Arc;

    /// A vault on keyring-core's in-memory mock store.
    pub(crate) fn mock_vault() -> CredentialVault {
        let store: Arc<CredentialStore> = keyring_core::mock::Store::new().unwrap();
        CredentialVault::open(&*store, "com.typefaced.desktop.test", "unit").unwrap()
    }

    #[test]
    fn stores_checks_reads_and_deletes_a_key() {
        let vault = mock_vault();
        assert!(!vault.has_key().unwrap());
        assert_eq!(vault.key().unwrap_err(), VaultError::NoKey);

        vault.set_key("test-key-not-real").unwrap();
        assert!(vault.has_key().unwrap());
        assert_eq!(vault.key().unwrap(), "test-key-not-real");

        vault.set_key("test-key-not-real-2").unwrap();
        assert_eq!(vault.key().unwrap(), "test-key-not-real-2");

        vault.delete_key().unwrap();
        assert!(!vault.has_key().unwrap());
        vault.delete_key().unwrap();
    }

    #[test]
    fn rejects_keys_that_cannot_be_a_header_value() {
        let vault = mock_vault();
        for key in [
            String::new(),
            "with space".to_owned(),
            "line\nbreak".to_owned(),
            "tab\t".to_owned(),
            "non-ascii-é".to_owned(),
            "x".repeat(MAX_KEY_BYTES + 1),
        ] {
            assert_eq!(vault.set_key(&key), Err(VaultError::InvalidKey));
        }
        assert!(!vault.has_key().unwrap());
        vault.set_key(&"x".repeat(MAX_KEY_BYTES)).unwrap();
    }

    #[test]
    fn store_errors_are_reported_without_secret_data() {
        let vault = mock_vault();
        vault.set_key("test-key-not-real").unwrap();
        let mock: &keyring_core::mock::Cred = vault.entry.as_any().downcast_ref().unwrap();

        mock.set_error(KeyringError::BadEncoding(b"test-key-not-real".to_vec()));
        let error = vault.has_key().unwrap_err();
        assert_eq!(
            error,
            VaultError::Store("Password data is not valid UTF-8".into())
        );
        assert!(!format!("{error:?}").contains("test-key"));

        mock.set_error(KeyringError::NotSupportedByStore("locked".into()));
        assert!(matches!(vault.delete_key(), Err(VaultError::Store(_))));
        mock.set_error(KeyringError::NotSupportedByStore("locked".into()));
        assert!(matches!(
            vault.set_key("test-key-not-real"),
            Err(VaultError::Store(_))
        ));
    }

    #[cfg(not(windows))]
    #[test]
    fn has_no_os_store_outside_windows() {
        assert_eq!(
            CredentialVault::production().unwrap_err(),
            VaultError::Unsupported
        );
    }

    /// Round trip through Windows Credential Manager on a test-only entry: write,
    /// re-open, read, delete. The entry is deleted on drop, even when an assert fails.
    #[cfg(windows)]
    #[test]
    fn round_trips_through_windows_credential_manager() {
        struct Cleanup(CredentialVault);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = self.0.delete_key();
            }
        }

        const SERVICE: &str = "com.typefaced.desktop.test";
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let account = format!("openrouter-api-key-{}-{nanos}", std::process::id());
        assert_ne!(SERVICE, PRODUCTION_SERVICE);
        assert_ne!(account, PRODUCTION_ACCOUNT);

        let writer = Cleanup(CredentialVault::os(SERVICE, &account).unwrap());
        assert!(!writer.0.has_key().unwrap());
        writer.0.set_key("test-key-not-real").unwrap();

        let reader = CredentialVault::os(SERVICE, &account).unwrap();
        assert!(reader.has_key().unwrap());
        assert_eq!(reader.key().unwrap(), "test-key-not-real");

        reader.delete_key().unwrap();
        assert!(!writer.0.has_key().unwrap());
    }
}
