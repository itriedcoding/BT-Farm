use crate::error::{CoreError, Result};
use base64::Engine;
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Key/value store for API keys and tokens.
///
/// On Windows the payload is sealed with DPAPI (`CryptProtectData`), so it is
/// bound to the logged-in Windows user and cannot be read by copying the file to
/// another machine or account. Other platforms fall back to a 0600 file and say
/// so loudly, because the shipped product targets Windows.
pub struct SecretStore {
    path: PathBuf,
    cache: Mutex<BTreeMap<String, String>>,
}

impl std::fmt::Debug for SecretStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretStore")
            .field("path", &self.path)
            .field("method", &self.method())
            .finish()
    }
}

impl SecretStore {
    pub fn new(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let path = dir.join("credentials.bin");
        let cache = Mutex::new(BTreeMap::new());
        let store = Self { path, cache };
        if store.path.exists() {
            let raw = fs::read(&store.path)?;
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(String::from_utf8_lossy(&raw).trim())
                .map_err(|e| CoreError::Secrets(format!("credential file is corrupt: {e}")))?;
            let plain = unseal(&decoded)?;
            match serde_json::from_slice::<BTreeMap<String, String>>(&plain) {
                Ok(map) => *store.cache.lock() = map,
                Err(e) => {
                    return Err(CoreError::Secrets(format!(
                        "credential file could not be decoded ({e}); delete {} and re-enter the keys",
                        store.path.display()
                    )))
                }
            }
        }
        Ok(store)
    }

    /// "dpapi" on Windows, "file-0600" elsewhere.
    pub fn method(&self) -> &'static str {
        if cfg!(windows) {
            "dpapi"
        } else {
            "file-0600"
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.cache.lock().get(key).cloned()
    }

    pub fn has(&self, key: &str) -> bool {
        self.cache.lock().contains_key(key)
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return self.delete(key);
        }
        {
            let mut cache = self.cache.lock();
            cache.insert(key.to_string(), trimmed.to_string());
        }
        self.persist()
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        {
            let mut cache = self.cache.lock();
            cache.remove(key);
        }
        self.persist()
    }

    /// Names only — values never leave this module.
    pub fn keys(&self) -> Vec<String> {
        self.cache.lock().keys().cloned().collect()
    }

    fn persist(&self) -> Result<()> {
        let json = {
            let cache = self.cache.lock();
            serde_json::to_vec(&Value::Object(
                cache
                    .iter()
                    .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                    .collect(),
            ))
            .map_err(|e| CoreError::Secrets(e.to_string()))?
        };
        let sealed = seal(&json)?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(sealed);
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, encoded)?;
        restrict_permissions(&self.path)?;
        Ok(())
    }
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> Result<()> {
    // DPAPI already binds the file to the current Windows user.
    Ok(())
}

#[cfg(windows)]
fn seal(plain: &[u8]) -> Result<Vec<u8>> {
    dpapi::protect(plain)
}

#[cfg(windows)]
fn unseal(sealed: &[u8]) -> Result<Vec<u8>> {
    dpapi::unprotect(sealed)
}

#[cfg(not(windows))]
fn seal(plain: &[u8]) -> Result<Vec<u8>> {
    let mut out = b"BTFARM-PLAIN-1:".to_vec();
    out.extend_from_slice(plain);
    Ok(out)
}

#[cfg(not(windows))]
fn unseal(sealed: &[u8]) -> Result<Vec<u8>> {
    const PREFIX: &[u8] = b"BTFARM-PLAIN-1:";
    if sealed.starts_with(PREFIX) {
        Ok(sealed[PREFIX.len()..].to_vec())
    } else {
        Err(CoreError::Secrets(
            "credential file was written on Windows with DPAPI and cannot be read here; re-enter the keys on this machine".into(),
        ))
    }
}

#[cfg(windows)]
mod dpapi {
    use super::*;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    fn to_blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        }
    }

    pub fn protect(plain: &[u8]) -> Result<Vec<u8>> {
        let mut input = to_blob(plain);
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let ok = unsafe {
            CryptProtectData(
                &mut input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(CoreError::Secrets(format!(
                "Windows DPAPI could not encrypt the credential file: {}",
                std::io::Error::last_os_error()
            )));
        }
        let bytes = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
        unsafe {
            LocalFree(output.pbData as *mut core::ffi::c_void);
        }
        Ok(bytes)
    }

    pub fn unprotect(sealed: &[u8]) -> Result<Vec<u8>> {
        let mut input = to_blob(sealed);
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let ok = unsafe {
            CryptUnprotectData(
                &mut input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(CoreError::Secrets(format!(
                "Windows DPAPI could not decrypt the credential file: {}",
                std::io::Error::last_os_error()
            )));
        }
        let bytes = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
        unsafe {
            LocalFree(output.pbData as *mut core::ffi::c_void);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_values_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let store = SecretStore::new(dir.path()).unwrap();
        store.set("provider:e2b:api_key", "e2b_secret_value").unwrap();
        store.set("provider:boat:api_key", "boat_secret").unwrap();
        assert_eq!(store.get("provider:e2b:api_key").as_deref(), Some("e2b_secret_value"));

        let reopened = SecretStore::new(dir.path()).unwrap();
        assert_eq!(reopened.get("provider:boat:api_key").as_deref(), Some("boat_secret"));

        reopened.delete("provider:boat:api_key").unwrap();
        let third = SecretStore::new(dir.path()).unwrap();
        assert!(third.get("provider:boat:api_key").is_none());
        assert_eq!(third.keys(), vec!["provider:e2b:api_key".to_string()]);
    }

    #[test]
    fn empty_value_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let store = SecretStore::new(dir.path()).unwrap();
        store.set("k", "v").unwrap();
        store.set("k", "   ").unwrap();
        assert!(!store.has("k"));
    }
}
