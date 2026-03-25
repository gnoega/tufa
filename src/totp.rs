use std::{
    fmt::Display,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use totp_rs::TOTP;

use crate::{migration, totp_uri::TotpURI};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Algorithm {
    SHA1,
    SHA256,
    SHA512,
}
impl Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Algorithm::SHA1 => write!(f, "SHA1"),
            Algorithm::SHA256 => write!(f, "SHA256"),
            Algorithm::SHA512 => write!(f, "SHA512"),
        }
    }
}

impl Default for Algorithm {
    fn default() -> Self {
        Self::SHA1
    }
}

impl From<Algorithm> for totp_rs::Algorithm {
    fn from(a: Algorithm) -> Self {
        match a {
            Algorithm::SHA1 => totp_rs::Algorithm::SHA1,
            Algorithm::SHA256 => totp_rs::Algorithm::SHA256,
            Algorithm::SHA512 => totp_rs::Algorithm::SHA512,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotpEntry {
    pub name: String,
    pub issuer: Option<String>,
    pub secret: String,
    #[serde(default = "default_digits")]
    pub digits: u8,
    #[serde(default = "default_period")]
    pub period: u64,
    pub algorithm: Algorithm,
}

fn default_digits() -> u8 {
    6
}

fn default_period() -> u64 {
    30
}

impl TotpEntry {
    pub fn new(secret: impl Into<String>, name: impl Into<String>) -> Result<Self, TotpError> {
        Ok(Self {
            secret: normalize_secret(secret.into())?,
            name: name.into(),
            issuer: None,
            digits: 6,
            period: 30,
            algorithm: Algorithm::default(),
        })
    }

    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = Some(issuer.into());

        self
    }

    pub fn from_entry(
        secret: String,
        name: String,
        issuer: Option<String>,
    ) -> Result<Self, TotpError> {
        let mut totp = Self::new(secret, name)?;
        totp.issuer = issuer;
        Ok(totp)
    }

    pub fn from_uri(uri: &str) -> Result<Self, TotpError> {
        let rest = uri
            .trim()
            .strip_prefix("otpauth://totp/")
            .ok_or(TotpError::InvalidUri("missing otpauth://totp/ prefix"))?;

        let (label, query) = rest
            .split_once('?')
            .ok_or(TotpError::InvalidUri("missing query string"))?;

        let label =
            urlencoding::decode(label).map_err(|_| TotpError::InvalidUri("bad label encoding"))?;

        let (issuer_from_label, name) = match label.split_once(':') {
            Some((i, n)) => (Some(i.trim().to_string()), n.trim().to_string()),
            None => (None, label.trim().to_string()),
        };

        let mut secret = None;
        let mut issuer = issuer_from_label;
        let mut digits = 6u8;
        let mut period = 30u64;
        let mut algorithm = Algorithm::SHA1;

        for param in query.split('&') {
            let (k, v) = param
                .split_once('=')
                .ok_or(TotpError::InvalidUri("bad query param"))?;
            let v =
                urlencoding::decode(v).map_err(|_| TotpError::InvalidUri("bad param encoding"))?;

            match k {
                "secret" => secret = Some(normalize_secret(v.into_owned())?),
                "issuer" => issuer = Some(v.into_owned()),
                "digits" => {
                    digits = v
                        .parse()
                        .map_err(|_| TotpError::InvalidUri("bad digits value"))?
                }
                "period" => {
                    period = v
                        .parse()
                        .map_err(|_| TotpError::InvalidUri("bad period value"))?
                }
                "algorithm" => {
                    algorithm = match v.as_ref() {
                        "SHA1" => Algorithm::SHA1,
                        "SHA256" => Algorithm::SHA256,
                        "SHA512" => Algorithm::SHA512,
                        _ => return Err(TotpError::InvalidUri("unknown algorithm")),
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            name,
            issuer,
            secret: secret.ok_or(TotpError::InvalidUri("missing secret"))?,
            digits,
            period,
            algorithm,
        })
    }

    pub fn to_uri(&self) -> TotpURI {
        let label = match &self.issuer {
            Some(i) => format!(
                "{}:{}",
                urlencoding::encode(i),
                urlencoding::encode(&self.name)
            ),
            None => urlencoding::encode(&self.name).into_owned(),
        };

        let mut uri = TotpURI::new(format!(
            "otpauth://totp/{}?secret={}&digits={}&period={}&algorithm={}",
            label, self.secret, self.digits, self.period, self.algorithm
        ));

        if let Some(issuer) = &self.issuer {
            uri.push_str(&format!("&issuer={}", urlencoding::encode(issuer)));
        }

        uri
    }

    pub fn generate_otp(&self) -> Result<String, TotpError> {
        let secret = base32::decode(base32::Alphabet::Rfc4648 { padding: false }, &self.secret)
            .ok_or(TotpError::InvalidSecret)?;

        TOTP::new_unchecked(
            self.algorithm.into(),
            self.digits as usize,
            1,
            self.period,
            secret,
        )
        .generate_current()
        .map_err(|e| TotpError::GenerationFailed(e.to_string()))
    }
    pub fn display_name(&self) -> String {
        match &self.issuer {
            Some(issuer) => format!("{issuer}:{}", self.name),
            None => self.name.clone(),
        }
    }
}

pub enum TotpError {
    InvalidSecret,
    InvalidUri(&'static str),
    GenerationFailed(String),
    DuplicatedSecret,
}

impl Display for TotpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TotpError::InvalidSecret => "invalid secret",
            TotpError::GenerationFailed(e) => return write!(f, "generation failed: {e}"),
            TotpError::InvalidUri(reason) => return write!(f, "invalid otpauth URI: {reason}"),
            TotpError::DuplicatedSecret => {
                return write!(f, "account with this secret already exists");
            }
        })
    }
}

pub fn totp_ttl() -> u64 {
    const TIME_STEP: u64 = 30;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time is before Unix epoch")
        .as_secs();
    TIME_STEP - (now % TIME_STEP)
}

pub fn normalize_secret(raw: String) -> Result<String, TotpError> {
    let normalize: String = raw
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect();

    base32::decode(base32::Alphabet::Rfc4648 { padding: false }, &normalize)
        .ok_or(TotpError::InvalidSecret)?;

    Ok(normalize)
}

pub fn parse_uri(uri: &str) -> Result<Vec<TotpEntry>, TotpError> {
    if uri.starts_with("otpauth://totp/") {
        TotpEntry::from_uri(uri).map(|e| vec![e])
    } else if uri.starts_with("otpauth-migration://") {
        migration::parse(uri)
    } else {
        Err(TotpError::InvalidUri("unrecognized URI scheme"))
    }
}
