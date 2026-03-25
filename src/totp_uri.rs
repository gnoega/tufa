use qrcode::{QrCode, render::unicode};
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, PartialEq)]
pub struct TotpURI(String);

impl TotpURI {
    pub fn new(uri: impl Into<String>) -> Self {
        Self(uri.into())
    }

    pub fn to_qrcode_string(&self) -> Result<String, qrcode::types::QrError> {
        let qr = QrCode::new(self.0.as_bytes())?;
        Ok(qr
            .render::<unicode::Dense1x2>()
            .dark_color(unicode::Dense1x2::Dark)
            .light_color(unicode::Dense1x2::Light)
            .build())
    }
}

impl Deref for TotpURI {
    type Target = String;
    fn deref(&self) -> &String {
        &self.0
    }
}

impl DerefMut for TotpURI {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<String> for TotpURI {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::fmt::Display for TotpURI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
