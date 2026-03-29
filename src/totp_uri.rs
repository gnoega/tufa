use qrcode::{QrCode, render::unicode};
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, PartialEq)]
pub struct TotpURI(String);

impl TotpURI {
    pub fn new(uri: impl Into<String>) -> Self {
        Self(uri.into())
    }

    pub fn to_qrcode(&self) -> Result<QrCode, qrcode::types::QrError> {
        QrCode::with_error_correction_level(self.0.as_bytes(), qrcode::EcLevel::L)
    }

    pub fn to_qrcode_rendered(&self) -> Result<(String, u16, u16), qrcode::types::QrError> {
        let qr = self.to_qrcode()?;
        let s = qr
            .render::<unicode::Dense1x2>()
            .dark_color(unicode::Dense1x2::Dark)
            .light_color(unicode::Dense1x2::Light)
            .quiet_zone(false)
            .build();

        let lines: Vec<&str> = s.lines().collect();
        let height = lines.len() as u16;
        let width = lines.first().map(|l| l.chars().count() as u16).unwrap_or(0);

        Ok((s, width, height))
    }

    pub fn to_qrcode_string(&self) -> Result<String, qrcode::types::QrError> {
        Ok(self.to_qrcode_rendered()?.0)
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
