use std::{fs, path::PathBuf};

use aes_gcm::{
    Aes256Gcm, Key, KeyInit, Nonce,
    aead::{Aead, OsRng, rand_core::RngCore},
};
use argon2::{Argon2, Params};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::totp::TotpEntry;

const EXTENSION_NAME: &str = ".2fa";

const MAGIC: &[u8; 4] = b"ENCR";

const VERSION: u8 = 1;

const ALGO_AES256GCM: u8 = 0x01;

const SALT_LEN: usize = 32;

const NONCE_LEN: usize = 12;

const TAG_LEN: usize = 16;

const ARGON2_MEM_KB: u32 = 19_456; // 19 MB
const ARGON2_ITERS: u32 = 2;
const ARGON2_LANES: u32 = 1;

/// ─── Header Layout ────────────────────────────────────────────────────────────
///
///  Offset  Size   Field
///  ──────  ─────  ──────────────────────────────────────────
///  0       4      Magic bytes ("ENCR")
///  4       1      Version
///  5       1      Algorithm ID
///  6       4      Argon2 memory cost (u32 LE)
///  10      4      Argon2 iterations  (u32 LE)
///  14      4      Argon2 parallelism (u32 LE)
///  18      32     Salt
///  50      12     Nonce / IV
///  62      N      Ciphertext (includes 16-byte AEAD tag at the end)
///
/// Total fixed header = 62 bytes
const HEADER_LEN: usize = 4 + 1 + 1 + 4 + 4 + 4 + SALT_LEN + NONCE_LEN;

#[derive(Zeroize, ZeroizeOnDrop)]
struct DerivedKey([u8; 32]);

#[derive(Debug, Serialize, Deserialize)]
pub struct VaultEntry {
    name: String,
    issuer: Option<String>,
    secret: String,
}

impl From<&TotpEntry> for VaultEntry {
    fn from(t: &TotpEntry) -> Self {
        Self {
            name: t.name.clone(),
            issuer: t.issuer.clone(),
            secret: t.secret.clone(),
        }
    }
}

#[derive(Debug)]
pub struct Vault {
    path: PathBuf,
}

impl Vault {
    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let path = Self::dir().join(format!("{}{}", name, EXTENSION_NAME));

        Self { path }
    }

    pub fn dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("tufa")
    }

    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    pub fn list_all() -> Vec<String> {
        let Ok(entries) = fs::read_dir(Self::dir()) else {
            return vec![];
        };

        entries
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                if path.extension()?.to_str()? == &EXTENSION_NAME[1..] {
                    path.file_name()?.to_str().map(str::to_string)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn load(&self, password: &[u8]) -> Result<Vec<TotpEntry>, VaultError> {
        if !self.path.exists() {
            return Err(VaultError::NotFound);
        }

        let encrypted = fs::read(&self.path).map_err(VaultError::Io)?;
        let plaintext = self.decrypt(&encrypted, password)?;

        let entries: Vec<VaultEntry> = serde_json::from_slice(&plaintext)
            .map_err(|e| VaultError::SerializationFailed(e.to_string()))?;

        entries
            .into_iter()
            .map(|e| TotpEntry::from_entry(e.secret, e.name, e.issuer))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| VaultError::SerializationFailed(e.to_string()))
    }

    pub fn save(&self, accounts: &[TotpEntry], password: &[u8]) -> Result<(), VaultError> {
        let entries: Vec<VaultEntry> = accounts.iter().map(VaultEntry::from).collect();
        let plaintext = serde_json::to_vec(&entries)
            .map_err(|e| VaultError::SerializationFailed(e.to_string()))?;

        let encrypted = self.encrypt(&plaintext, password)?;

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(VaultError::Io)?;
        }
        fs::write(&self.path, encrypted).map_err(VaultError::Io)?;
        Ok(())
    }

    fn derive_key(&self, password: &[u8], salt: &[u8]) -> Result<DerivedKey, VaultError> {
        self.derive_key_with_params(password, salt, ARGON2_MEM_KB, ARGON2_ITERS, ARGON2_LANES)
    }

    fn derive_key_with_params(
        &self,
        password: &[u8],
        salt: &[u8],
        mem_kb: u32,
        iters: u32,
        lanes: u32,
    ) -> Result<DerivedKey, VaultError> {
        let params = Params::new(mem_kb, iters, lanes, Some(32))
            .map_err(|e| VaultError::CryptoError(e.to_string()))?;
        let mut key = DerivedKey([0u8; 32]);

        Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
            .hash_password_into(password, salt, &mut key.0)
            .map_err(|e| VaultError::CryptoError(e.to_string()))?;

        Ok(key)
    }

    fn encrypt(&self, plaintext: &[u8], password: &[u8]) -> Result<Vec<u8>, VaultError> {
        let mut salt = [0u8; SALT_LEN];
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut salt);
        OsRng.fill_bytes(&mut nonce_bytes);

        let key = self.derive_key(password, &salt)?;

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key.0));
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| VaultError::EncryptionFailed)?;

        Ok(self.to_bytes(&salt, &nonce_bytes, &ciphertext))
    }

    fn to_bytes(
        &self,
        salt: &[u8; SALT_LEN],
        nonce: &[u8; NONCE_LEN],
        ciphertext: &[u8],
    ) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + ciphertext.len());

        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.push(ALGO_AES256GCM);
        out.extend_from_slice(&ARGON2_MEM_KB.to_le_bytes());
        out.extend_from_slice(&ARGON2_ITERS.to_le_bytes());
        out.extend_from_slice(&ARGON2_LANES.to_le_bytes());
        out.extend_from_slice(salt);
        out.extend_from_slice(nonce);
        out.extend_from_slice(ciphertext);

        out
    }

    fn decrypt(&self, data: &[u8], password: &[u8]) -> Result<Vec<u8>, VaultError> {
        if data.len() < HEADER_LEN + TAG_LEN + 1 {
            return Err(VaultError::TooShort);
        }
        let mut offset = 0;
        if &data[offset..offset + 4] != MAGIC {
            return Err(VaultError::BadMagic);
        }

        offset += 4;

        let version = data[offset];
        if version != VERSION {
            return Err(VaultError::UnsupportedVersion(version));
        }
        offset += 1;

        let algo_id = data[offset];
        if algo_id != ALGO_AES256GCM {
            return Err(VaultError::UnsupportedAlgorithm(algo_id));
        }
        offset += 1;

        let mem_kb = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let iters = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let lanes = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        offset += 4;

        let salt: [u8; SALT_LEN] = data[offset..offset + SALT_LEN].try_into().unwrap();
        offset += SALT_LEN;
        let nonce_bytes: [u8; NONCE_LEN] = data[offset..offset + NONCE_LEN].try_into().unwrap();
        offset += NONCE_LEN;

        let ciphertext = &data[offset..];

        let key = self.derive_key_with_params(password, &salt, mem_kb, iters, lanes)?;

        // AEAD: verifies auth tag BEFORE returning any plaintext
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key.0));
        let nonce = Nonce::from_slice(&nonce_bytes);
        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| VaultError::WrongPassword)
    }
}

#[derive(Debug)]
pub enum VaultError {
    NotFound,
    AccountNotFound(String),
    TooShort,
    BadMagic,
    UnsupportedVersion(u8),
    UnsupportedAlgorithm(u8),
    WrongPassword,
    Io(std::io::Error),
    SerializationFailed(String),
    CryptoError(String),
    EncryptionFailed,
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultError::NotFound => f.write_str("vault file not found"),
            VaultError::TooShort => f.write_str("file too short to be a valid vault"),
            VaultError::BadMagic => f.write_str("invalid magic bytes — not a vault file"),
            VaultError::UnsupportedVersion(v) => write!(f, "unsupported vault version: {v}"),
            VaultError::UnsupportedAlgorithm(a) => write!(f, "unsupported algorithm id: 0x{a:02X}"),
            VaultError::WrongPassword => f.write_str("wrong password or file is corrupted"),
            VaultError::Io(e) => write!(f, "io error: {e}"),
            VaultError::CryptoError(e) => write!(f, "crypto error: {e}"),
            VaultError::SerializationFailed(e) => write!(f, "serialization failed: {e}"),
            VaultError::EncryptionFailed => f.write_str("encryption failed"),
            VaultError::AccountNotFound(e) => write!(f, "Account '{e}' not found"),
        }
    }
}

impl std::error::Error for VaultError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::matches;
    use tempfile::TempDir;

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// A vault backed by a temp directory that is deleted when `_dir` drops.
    fn temp_vault() -> (Vault, TempDir) {
        let dir = TempDir::new().unwrap();
        let vault = Vault::with_path(dir.path().join("vault.2fa"));
        (vault, dir)
    }

    /// Minimal valid plaintext — a JSON array of one entry.
    fn sample_plaintext() -> Vec<u8> {
        br#"[{"name":"Alice","issuer":"GitHub","secret":"JBSWY3DPEHPK3PXP"}]"#.to_vec()
    }

    // ── Encrypt / Decrypt round-trip ──────────────────────────────────────────

    #[test]
    fn round_trip_basic() {
        let (vault, _dir) = temp_vault();
        let plain = sample_plaintext();
        let password = b"correct-horse-battery-staple";

        let encrypted = vault.encrypt(&plain, password).unwrap();
        let recovered = vault.decrypt(&encrypted, password).unwrap();

        assert_eq!(recovered, plain);
    }

    #[test]
    fn round_trip_empty_plaintext() {
        // Empty payload is valid — save/load of zero accounts
        let (vault, _dir) = temp_vault();
        let plain = b"[]";
        let password = b"pass";

        let encrypted = vault.encrypt(plain, password).unwrap();
        let recovered = vault.decrypt(&encrypted, password).unwrap();

        assert_eq!(&recovered, plain);
    }

    #[test]
    fn round_trip_binary_plaintext() {
        // Crypto layer should handle arbitrary bytes, not just valid UTF-8
        let (vault, _dir) = temp_vault();
        let plain: Vec<u8> = (0u8..=255).collect();
        let password = b"pass";

        let encrypted = vault.encrypt(&plain, password).unwrap();
        let recovered = vault.decrypt(&encrypted, password).unwrap();

        assert_eq!(recovered, plain);
    }

    // ── Ciphertext is non-deterministic ───────────────────────────────────────

    #[test]
    fn same_input_produces_different_ciphertext() {
        // Fresh salt + nonce on every call — identical inputs must never produce
        // identical output.
        let (vault, _dir) = temp_vault();
        let plain = sample_plaintext();
        let password = b"pass";

        let a = vault.encrypt(&plain, password).unwrap();
        let b = vault.encrypt(&plain, password).unwrap();

        assert_ne!(a, b, "two encryptions of the same plaintext must differ");
    }

    // ── Header layout ─────────────────────────────────────────────────────────

    #[test]
    fn header_starts_with_magic() {
        let (vault, _dir) = temp_vault();
        let encrypted = vault.encrypt(b"x", b"pass").unwrap();
        assert_eq!(&encrypted[..4], b"ENCR");
    }

    #[test]
    fn header_version_byte_is_one() {
        let (vault, _dir) = temp_vault();
        let encrypted = vault.encrypt(b"x", b"pass").unwrap();
        assert_eq!(encrypted[4], 1u8);
    }

    #[test]
    fn header_algo_byte_is_aes256gcm() {
        let (vault, _dir) = temp_vault();
        let encrypted = vault.encrypt(b"x", b"pass").unwrap();
        assert_eq!(encrypted[5], ALGO_AES256GCM);
    }

    #[test]
    fn header_length_matches_constant() {
        // The encrypted output must be at least HEADER_LEN + TAG_LEN + plaintext bytes
        let (vault, _dir) = temp_vault();
        let plain = b"hello";
        let encrypted = vault.encrypt(plain, b"pass").unwrap();

        assert!(encrypted.len() >= HEADER_LEN + TAG_LEN + plain.len());
    }

    // ── Wrong password ────────────────────────────────────────────────────────

    #[test]
    fn wrong_password_returns_wrong_password_error() {
        let (vault, _dir) = temp_vault();
        let encrypted = vault.encrypt(b"secret", b"right").unwrap();
        let result = vault.decrypt(&encrypted, b"wrong");

        assert!(
            matches!(result, Err(VaultError::WrongPassword)),
            "expected WrongPassword, got: {:?}",
            result.err()
        );
    }

    #[test]
    fn empty_password_differs_from_non_empty() {
        let (vault, _dir) = temp_vault();
        let encrypted = vault.encrypt(b"data", b"password").unwrap();

        assert!(matches!(
            vault.decrypt(&encrypted, b""),
            Err(VaultError::WrongPassword)
        ));
    }

    // ── Tamper detection ──────────────────────────────────────────────────────

    #[test]
    fn tampered_ciphertext_detected() {
        let (vault, _dir) = temp_vault();
        let mut encrypted = vault.encrypt(b"data", b"pass").unwrap();

        // Flip the last byte (inside the AEAD tag)
        let last = encrypted.len() - 1;
        encrypted[last] ^= 0xFF;

        assert!(matches!(
            vault.decrypt(&encrypted, b"pass"),
            Err(VaultError::WrongPassword)
        ));
    }

    #[test]
    fn tampered_nonce_detected() {
        let (vault, _dir) = temp_vault();
        let mut encrypted = vault.encrypt(b"data", b"pass").unwrap();

        // Nonce starts at offset 50 (after magic+version+algo+3×u32+salt)
        encrypted[50] ^= 0xFF;

        assert!(matches!(
            vault.decrypt(&encrypted, b"pass"),
            Err(VaultError::WrongPassword)
        ));
    }

    #[test]
    fn tampered_salt_detected() {
        // Changing the salt causes key re-derivation to produce a different key,
        // which then fails the AEAD auth tag check.
        let (vault, _dir) = temp_vault();
        let mut encrypted = vault.encrypt(b"data", b"pass").unwrap();

        // Salt starts at offset 18
        encrypted[18] ^= 0xFF;

        assert!(matches!(
            vault.decrypt(&encrypted, b"pass"),
            Err(VaultError::WrongPassword)
        ));
    }

    // ── Header validation errors ──────────────────────────────────────────────

    #[test]
    fn too_short_rejected() {
        let (vault, _dir) = temp_vault();
        let result = vault.decrypt(&[0u8; 10], b"pass");
        assert!(matches!(result, Err(VaultError::TooShort)));
    }

    #[test]
    fn bad_magic_rejected() {
        let (vault, _dir) = temp_vault();
        let mut encrypted = vault.encrypt(b"data", b"pass").unwrap();
        encrypted[0] = 0x00; // corrupt magic
        let result = vault.decrypt(&encrypted, b"pass");
        assert!(matches!(result, Err(VaultError::BadMagic)));
    }

    #[test]
    fn wrong_version_rejected() {
        let (vault, _dir) = temp_vault();
        let mut encrypted = vault.encrypt(b"data", b"pass").unwrap();
        encrypted[4] = 0xFF; // unsupported version
        let result = vault.decrypt(&encrypted, b"pass");
        assert!(matches!(result, Err(VaultError::UnsupportedVersion(0xFF))));
    }

    #[test]
    fn wrong_algo_rejected() {
        let (vault, _dir) = temp_vault();
        let mut encrypted = vault.encrypt(b"data", b"pass").unwrap();
        encrypted[5] = 0xFF; // unsupported algorithm
        let result = vault.decrypt(&encrypted, b"pass");
        assert!(matches!(
            result,
            Err(VaultError::UnsupportedAlgorithm(0xFF))
        ));
    }

    // ── save / load (disk round-trip) ─────────────────────────────────────────

    #[test]
    fn save_produces_encrypted_file() {
        let (vault, _dir) = temp_vault();
        let password = b"pass";

        vault.save(&[], password).unwrap();

        let raw = fs::read(&vault.path).unwrap();
        // Must start with magic — plaintext JSON would start with `[`
        assert_eq!(&raw[..4], b"ENCR", "file on disk should be encrypted");
    }

    #[test]
    fn load_fails_when_file_missing() {
        let (vault, _dir) = temp_vault();
        let result = vault.load(b"pass");
        assert!(matches!(result, Err(VaultError::NotFound)));
    }

    #[test]
    fn load_fails_with_wrong_password() {
        let (vault, _dir) = temp_vault();
        vault.save(&[], b"right").unwrap();
        let result = vault.load(b"wrong");
        assert!(matches!(result, Err(VaultError::WrongPassword)));
    }

    #[test]
    fn save_and_load_empty_accounts() {
        let (vault, _dir) = temp_vault();
        vault.save(&[], b"pass").unwrap();
        let loaded = vault.load(b"pass").unwrap();
        assert!(loaded.is_empty());
    }

    // ── Key derivation ────────────────────────────────────────────────────────

    #[test]
    fn same_password_and_salt_derives_same_key() {
        let (vault, _dir) = temp_vault();
        let password = b"password";
        let salt = [0x42u8; SALT_LEN];

        let a = vault.derive_key(password, &salt).unwrap();
        let b = vault.derive_key(password, &salt).unwrap();

        assert_eq!(a.0, b.0);
    }

    #[test]
    fn different_salt_produces_different_key() {
        let (vault, _dir) = temp_vault();
        let password = b"password";
        let salt_a = [0x01u8; SALT_LEN];
        let salt_b = [0x02u8; SALT_LEN];

        let a = vault.derive_key(password, &salt_a).unwrap();
        let b = vault.derive_key(password, &salt_b).unwrap();

        assert_ne!(a.0, b.0);
    }

    #[test]
    fn different_password_produces_different_key() {
        let (vault, _dir) = temp_vault();
        let salt = [0x42u8; SALT_LEN];

        let a = vault.derive_key(b"password1", &salt).unwrap();
        let b = vault.derive_key(b"password2", &salt).unwrap();

        assert_ne!(a.0, b.0);
    }

    // ── exists() ──────────────────────────────────────────────────────────────

    #[test]
    fn exists_false_before_save() {
        let (vault, _dir) = temp_vault();
        assert!(!vault.exists());
    }

    #[test]
    fn exists_true_after_save() {
        let (vault, _dir) = temp_vault();
        vault.save(&[], b"pass").unwrap();
        assert!(vault.exists());
    }
}
