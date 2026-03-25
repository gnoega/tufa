use prost::Message;

use crate::totp::{Algorithm, TotpEntry, TotpError};

// ---------------------------------------------------------------------------
// Protobuf schema — reverse engineered from Google Authenticator
// ---------------------------------------------------------------------------

#[derive(Message)]
struct MigrationPayload {
    #[prost(message, repeated, tag = "1")]
    otp_parameters: Vec<OtpParameters>,
}

#[derive(Message)]
struct OtpParameters {
    /// Raw secret bytes (not base32 encoded)
    #[prost(bytes = "vec", tag = "1")]
    secret: Vec<u8>,
    #[prost(string, tag = "2")]
    name: String,
    #[prost(string, tag = "3")]
    issuer: String,
    #[prost(int32, tag = "4")]
    algorithm: i32,
    #[prost(int32, tag = "5")]
    digits: i32,
    /// 1 = HOTP, 2 = TOTP
    #[prost(int32, tag = "6")]
    otp_type: i32,
}

// ---------------------------------------------------------------------------
// Algorithm mapping
// ---------------------------------------------------------------------------

// Google Authenticator's algorithm enum values
const GA_ALGO_SHA1: i32 = 1;
const GA_ALGO_SHA256: i32 = 2;
const GA_ALGO_SHA512: i32 = 3;

// Google Authenticator's digit enum values
const GA_DIGITS_SIX: i32 = 1;
const GA_DIGITS_EIGHT: i32 = 2;

const OTP_TYPE_TOTP: i32 = 2;

fn parse_algorithm(value: i32) -> Algorithm {
    match value {
        GA_ALGO_SHA256 => Algorithm::SHA256,
        GA_ALGO_SHA512 => Algorithm::SHA512,
        _ => Algorithm::SHA1, // GA_ALGO_SHA1 or unknown — fall back to SHA1
    }
}

fn parse_digits(value: i32) -> u8 {
    match value {
        GA_DIGITS_EIGHT => 8,
        _ => 6, // GA_DIGITS_SIX or unknown — fall back to 6
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse a `otpauth-migration://offline?data=...` URI exported by Google
/// Authenticator into a list of [`TotpEntry`] values.
///
/// HOTP entries are silently skipped since we only support TOTP.
pub fn parse(uri: &str) -> Result<Vec<TotpEntry>, TotpError> {
    let data = extract_data(uri)?;
    println!("migration.rs: data: {:?}", data);
    let payload = decode_payload(&data)?;
    println!("migration.rs: payload: {:?}", payload);

    payload
        .otp_parameters
        .into_iter()
        .filter(|p| p.otp_type == OTP_TYPE_TOTP)
        .map(convert_entry)
        .collect()
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Extract and base64-decode the `data` query parameter from the URI.
fn extract_data(uri: &str) -> Result<Vec<u8>, TotpError> {
    let query = uri
        .strip_prefix("otpauth-migration://offline?")
        .ok_or(TotpError::InvalidUri(
            "missing otpauth-migration://offline? prefix",
        ))?;

    // There may be multiple query params in theory, find `data=`
    let data_param = query
        .split('&')
        .find(|s| s.starts_with("data="))
        .ok_or(TotpError::InvalidUri("missing data param"))?
        .strip_prefix("data=")
        .unwrap();

    let data =
        urlencoding::decode(data_param).map_err(|_| TotpError::InvalidUri("bad data encoding"))?;

    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD
        .decode(data.as_ref())
        .map_err(|_| TotpError::InvalidUri("bad base64 in data param"))
}

/// Decode the protobuf payload from raw bytes.
fn decode_payload(bytes: &[u8]) -> Result<MigrationPayload, TotpError> {
    MigrationPayload::decode(bytes).map_err(|_| TotpError::InvalidUri("bad protobuf payload"))
}

/// Convert a single [`OtpParameters`] entry into a [`TotpEntry`].
fn convert_entry(p: OtpParameters) -> Result<TotpEntry, TotpError> {
    // The secret comes as raw bytes — re-encode to base32 for our canonical form
    let secret = base32::encode(base32::Alphabet::Rfc4648 { padding: false }, &p.secret);

    Ok(TotpEntry {
        name: p.name,
        issuer: if p.issuer.is_empty() {
            None
        } else {
            Some(p.issuer)
        },
        secret,
        digits: parse_digits(p.digits),
        period: 30, // GA migration format doesn't encode period, TOTP standard is 30s
        algorithm: parse_algorithm(p.algorithm),
    })
}
