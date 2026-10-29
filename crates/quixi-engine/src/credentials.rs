//! Authenticated credential encryption; the encryption key lives in the OS vault.
use aes_gcm::{
    aead::{rand_core::RngCore, Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

fn key(path: &Path, create: bool) -> Result<Vec<u8>, String> {
    // Disposable test profiles never read or change the real account's vault.
    if cfg!(test) || quixi_proto::paths::test_profile().is_some() {
        let p = path.with_file_name("test-credentials.key");
        if p.exists() {
            return fs::read(p).map_err(|e| e.to_string());
        }
        if !create {
            return Err("Test credential key is missing".into());
        }
        let mut k = vec![0; 32];
        OsRng.fill_bytes(&mut k);
        super::storage::atomic_write(&p, &k).map_err(|e| e.to_string())?;
        return Ok(k);
    }
    let identity = format!("{:x}", Sha256::digest(path.to_string_lossy().as_bytes()));
    let entry =
        keyring::Entry::new("QuixiDesk credentials", &identity).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(s) => STANDARD.decode(s).map_err(|e| e.to_string()),
        Err(keyring::Error::NoEntry) if create => {
            let mut k = vec![0; 32];
            OsRng.fill_bytes(&mut k);
            entry
                .set_password(&STANDARD.encode(&k))
                .map_err(|e| e.to_string())?;
            Ok(k)
        }
        Err(e) => Err(format!(
            "Unlock your system credential store and try again: {e}"
        )),
    }
}
pub(crate) fn seal(path: &Path, mut document: Value) -> Result<Value, String> {
    let mut secrets = serde_json::Map::new();
    for provider in document["providers"].as_array_mut().into_iter().flatten() {
        let id = provider["id"].as_str().unwrap_or_default().to_string();
        let mut fields = serde_json::Map::new();
        for field in ["apiKeyRaw", "oauthAccessToken", "oauthRefreshToken"] {
            if let Some(v) = provider.as_object_mut().and_then(|p| p.remove(field)) {
                if v.as_str().is_some_and(|s| !s.is_empty()) {
                    fields.insert(field.into(), v);
                }
            }
        }
        if !fields.is_empty() {
            secrets.insert(id, Value::Object(fields));
        }
    }
    if !secrets.is_empty() {
        let k = key(path, true)?;
        let cipher = Aes256Gcm::new_from_slice(&k).map_err(|_| "Invalid credential key")?;
        let mut nonce = [0; 12];
        OsRng.fill_bytes(&mut nonce);
        let encrypted = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                serde_json::to_vec(&secrets).unwrap().as_ref(),
            )
            .map_err(|_| "Could not encrypt credentials")?;
        document["encryptedCredentials"] = json!({"version":1,"nonce":STANDARD.encode(nonce),"ciphertext":STANDARD.encode(encrypted)});
    }
    Ok(document)
}
pub(crate) fn open(path: &Path, mut document: Value) -> Result<Value, String> {
    if let Some(encrypted) = document.get("encryptedCredentials") {
        if encrypted["version"] != 1 {
            return Err("Unsupported credential format".into());
        }
        let decode = |field: &str| {
            STANDARD
                .decode(encrypted[field].as_str().unwrap_or_default())
                .map_err(|_| "Invalid credential encoding".to_string())
        };
        let nonce = decode("nonce")?;
        if nonce.len() != 12 {
            return Err("Invalid credential nonce".into());
        }
        let k = key(path, false)?;
        let cipher = Aes256Gcm::new_from_slice(&k).map_err(|_| "Invalid credential key")?;
        let bytes = cipher
            .decrypt(Nonce::from_slice(&nonce), decode("ciphertext")?.as_ref())
            .map_err(|_| "Credentials could not be decrypted. Existing file preserved.")?;
        let secrets: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        for provider in document["providers"].as_array_mut().into_iter().flatten() {
            if let Some(fields) = secrets[provider["id"].as_str().unwrap_or_default()].as_object() {
                provider.as_object_mut().unwrap().extend(fields.clone());
            }
        }
        document
            .as_object_mut()
            .unwrap()
            .remove("encryptedCredentials");
    }
    Ok(document)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_roundtrip_and_corruption_never_expose_secrets() {
        let dir = std::env::temp_dir().join(format!("quixi-vault-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("providers.json");
        let original = json!({"providers":[{"id":"custom","apiKeyRaw":"secret-api-key","oauthAccessToken":"access-token","oauthRefreshToken":"refresh-token"}]});
        let sealed = seal(&path, original.clone()).unwrap();
        let text = sealed.to_string();
        assert!(!text.contains("secret-api-key"));
        assert!(!text.contains("access-token"));
        assert_eq!(open(&path, sealed.clone()).unwrap(), original);
        let mut damaged = sealed;
        damaged["encryptedCredentials"]["ciphertext"] = json!(STANDARD.encode([0; 32]));
        assert!(open(&path, damaged).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
