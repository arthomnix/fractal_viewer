mod compat;

use base64::{engine::general_purpose, Engine};
use std::fmt::{Display, Formatter};
use crate::SHADER;

#[derive(Debug, serde::Deserialize)]
pub enum InvalidSettingsImportError {
    InvalidFormat,
    VersionMismatch,
    InvalidBase64,
    DeserialisationFailed,
}

impl InvalidSettingsImportError {
    fn to_str(&self) -> &str {
        match self {
            InvalidSettingsImportError::InvalidFormat => "Invalid settings string format",
            InvalidSettingsImportError::VersionMismatch => "Version mismatch or invalid format",
            InvalidSettingsImportError::InvalidBase64 => "Base64 decoding failed",
            InvalidSettingsImportError::DeserialisationFailed => "Deserialising data failed",
        }
    }
}

impl Display for InvalidSettingsImportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

impl std::error::Error for InvalidSettingsImportError {
    fn description(&self) -> &str {
        self.to_str()
    }
}

fn get_major_minor_version() -> String {
    let mut version_iterator = env!("CARGO_PKG_VERSION").split('.');
    format!(
        "{}.{}",
        version_iterator.next().unwrap(),
        version_iterator.next().unwrap()
    )
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CustomShaderData {
    pub(crate) equation: String,
    pub(crate) colour: String,
    pub(crate) additional: String,
}

impl CustomShaderData {
    pub(crate) fn shader(&self) -> String {
        SHADER
            .replace("REPLACE_FRACTAL_EQN", &self.equation)
            .replace("REPLACE_COLOR", &self.colour)
            + &self.additional
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct UserSettings {
    pub(crate) zoom: f32,
    pub(crate) centre: [f32; 2],
    pub(crate) iterations: i32,
    pub(crate) julia_set: bool,
    pub(crate) smoothen: bool,
    pub(crate) internal_black: bool,
    pub(crate) initial_value: [f32; 2],
    pub(crate) escape_threshold: f32,
    pub(crate) initial_c: bool,
    pub(crate) shader_data: CustomShaderData,
}

impl UserSettings {
    pub(crate) fn export_string(&self) -> String {
        let encoded = bincode::serialize(self).unwrap();
        format!(
            "{};{}",
            get_major_minor_version(),
            general_purpose::STANDARD.encode(encoded)
        )
    }

    pub(crate) fn import_string(string: &str) -> Result<Self, InvalidSettingsImportError> {
        let string = match url::Url::parse(string) {
            Ok(url) => url.query().unwrap_or_default().to_string(),
            Err(_) => string.to_string(),
        };

        if string.is_empty() {
            return Err(InvalidSettingsImportError::InvalidFormat);
        }

        let mut iterator = string.split(';');

        let major_minor_version = iterator
            .next()
            .ok_or(InvalidSettingsImportError::InvalidFormat)?;

        let base64 = iterator
            .next()
            .ok_or(InvalidSettingsImportError::InvalidFormat)?;

        let this_ver = get_major_minor_version();
        match major_minor_version {
            s if s == &this_ver => {
                let bytes = general_purpose::STANDARD
                    .decode(base64)
                    .map_err(|_| InvalidSettingsImportError::InvalidBase64)?;
                let result = bincode::deserialize::<'_, Self>(bytes.as_slice())
                    .map_err(|_| InvalidSettingsImportError::DeserialisationFailed)?;
                Ok(result)
            }
            "2.0" => Ok(compat::v2_0::UserSettings::import_string(base64)?.into()),
            "0.5" => Ok(compat::v0_5::UserSettings::import_string(base64)?.into()),
            "0.3" => Ok(compat::v0_3::UserSettings::import_string(base64)?.into()),
            "0.4" => Ok(compat::v0_4::UserSettings::import_string(base64)?.into()),
            _ => Err(InvalidSettingsImportError::VersionMismatch),
        }
    }
}

impl Default for CustomShaderData {
    fn default() -> Self {
        Self {
            equation: "csquare(z) + c".to_string(),
            colour: "hsv_rgb(vec3(log(n + 1.0) / log(f32(uniforms.iterations) + 1.0), 0.8, 0.8))".to_string(),
            additional: String::new(),
        }
    }
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            centre: [0.0, 0.0],
            iterations: 100,
            julia_set: false,
            smoothen: false,
            internal_black: true,
            initial_value: [0.0, 0.0],
            escape_threshold: 2.0,
            initial_c: false,
            shader_data: Default::default(),
        }
    }
}
