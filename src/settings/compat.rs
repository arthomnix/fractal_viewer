pub(crate) mod v0_3 {
    use crate::settings::InvalidSettingsImportError;

    use base64::engine::general_purpose;
    use base64::Engine;

    #[derive(Clone, serde::Serialize, serde::Deserialize)]
    pub(crate) struct UserSettings {
        zoom: f32,
        centre: [f32; 2],
        iterations: i32,
        equation: String,
        prev_equation: String,
        equation_valid: bool,
        julia_set: bool,
        initial_value: [f32; 2],
        escape_threshold: f32,
    }

    impl UserSettings {
        pub(crate) fn import_string(string: &str) -> Result<Self, InvalidSettingsImportError> {
            let bytes = general_purpose::STANDARD
                .decode(string)
                .map_err(|_| InvalidSettingsImportError::InvalidBase64)?;
            let result = bincode::deserialize::<'_, Self>(bytes.as_slice())
                .map_err(|_| InvalidSettingsImportError::DeserialisationFailed)?;
            Ok(result)
        }
    }

    impl Into<crate::settings::UserSettings> for UserSettings {
        fn into(self) -> crate::settings::UserSettings {
            crate::settings::UserSettings {
                zoom: self.zoom,
                centre: self.centre,
                iterations: self.iterations,
                equation: self.equation,
                colour:
                    "hsv_rgb(vec3(log(n + 1.0) / log(f32(uniforms.iterations) + 1.0), 0.8, 0.8))"
                        .to_string(),
                julia_set: self.julia_set,
                smoothen: false,
                internal_black: true,
                initial_value: self.initial_value,
                escape_threshold: self.escape_threshold,
                initial_c: false,
            }
        }
    }
}

pub(crate) mod v0_4 {
    use crate::settings::InvalidSettingsImportError;

    use base64::engine::general_purpose;
    use base64::Engine;

    #[derive(Clone, serde::Serialize, serde::Deserialize)]
    pub(crate) struct UserSettings {
        zoom: f32,
        centre: [f32; 2],
        iterations: i32,
        equation: String,
        prev_equation: String,
        colour: String,
        prev_colour: String,
        equation_valid: bool,
        julia_set: bool,
        smoothen: bool,
        internal_black: bool,
        initial_value: [f32; 2],
        escape_threshold: f32,
    }

    impl UserSettings {
        pub(crate) fn import_string(string: &str) -> Result<Self, InvalidSettingsImportError> {
            let bytes = general_purpose::STANDARD
                .decode(string)
                .map_err(|_| InvalidSettingsImportError::InvalidBase64)?;
            let result = bincode::deserialize::<'_, Self>(bytes.as_slice())
                .map_err(|_| InvalidSettingsImportError::DeserialisationFailed)?;
            Ok(result)
        }
    }

    impl Into<crate::settings::UserSettings> for UserSettings {
        fn into(self) -> crate::settings::UserSettings {
            crate::settings::UserSettings {
                zoom: self.zoom,
                centre: self.centre,
                iterations: self.iterations,
                equation: self.equation,
                colour: self.colour,
                julia_set: self.julia_set,
                smoothen: self.smoothen,
                internal_black: self.internal_black,
                initial_value: self.initial_value,
                escape_threshold: self.escape_threshold,
                initial_c: false,
            }
        }
    }
}

pub(crate) mod v0_5 {
    use crate::settings::InvalidSettingsImportError;

    use base64::engine::general_purpose;
    use base64::Engine;

    #[derive(Clone, serde::Serialize, serde::Deserialize)]
    pub(crate) struct UserSettings {
        zoom: f32,
        centre: [f32; 2],
        iterations: i32,
        equation: String,
        prev_equation: String,
        colour: String,
        prev_colour: String,
        equation_valid: bool,
        julia_set: bool,
        smoothen: bool,
        internal_black: bool,
        initial_value: [f32; 2],
        escape_threshold: f32,
        initial_c: bool,
    }

    impl UserSettings {
        pub(crate) fn import_string(string: &str) -> Result<Self, InvalidSettingsImportError> {
            let bytes = general_purpose::STANDARD
                .decode(string)
                .map_err(|_| InvalidSettingsImportError::InvalidBase64)?;
            let result = bincode::deserialize::<'_, Self>(bytes.as_slice())
                .map_err(|_| InvalidSettingsImportError::DeserialisationFailed)?;
            Ok(result)
        }
    }

    impl Into<crate::settings::UserSettings> for UserSettings {
        fn into(self) -> crate::settings::UserSettings {
            crate::settings::UserSettings {
                zoom: self.zoom,
                centre: self.centre,
                iterations: self.iterations,
                equation: self.equation,
                colour: self.colour,
                julia_set: self.julia_set,
                smoothen: self.smoothen,
                internal_black: self.internal_black,
                initial_value: self.initial_value,
                escape_threshold: self.escape_threshold,
                initial_c: self.initial_c,
            }
        }
    }
}
