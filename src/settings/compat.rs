pub(in crate::settings) mod v0_3 {
    use crate::settings::{CustomShaderData, InvalidSettingsImportError};

    use base64::engine::general_purpose;
    use base64::Engine;

    #[derive(Clone, serde::Serialize, serde::Deserialize)]
    pub(in crate::settings) struct UserSettings {
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
        pub(in crate::settings) fn import_base64(
            string: &str,
        ) -> Result<Self, InvalidSettingsImportError> {
            let bytes = general_purpose::STANDARD
                .decode(string)
                .map_err(|_| InvalidSettingsImportError::InvalidBase64)?;
            let result = bincode_next::serde::decode_from_slice::<Self, _>(
                bytes.as_slice(),
                bincode_next::config::legacy(),
            )
            .map_err(|_| InvalidSettingsImportError::DeserialisationFailed)?
            .0;
            Ok(result)
        }
    }

    impl From<UserSettings> for crate::settings::UserSettings {
        fn from(val: UserSettings) -> crate::settings::UserSettings {
            crate::settings::UserSettings {
                zoom: val.zoom,
                centre: val.centre,
                iterations: val.iterations,
                julia_set: val.julia_set,
                initial_value: val.initial_value,
                escape_threshold: val.escape_threshold,
                shader_data: CustomShaderData {
                    equation: val.equation,
                    ..Default::default()
                },
                ..Default::default()
            }
        }
    }
}

pub(in crate::settings) mod v0_4 {
    use crate::settings::{CustomShaderData, InvalidSettingsImportError};

    use base64::engine::general_purpose;
    use base64::Engine;

    #[derive(Clone, serde::Serialize, serde::Deserialize)]
    pub(in crate::settings) struct UserSettings {
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
        pub(in crate::settings) fn import_base64(
            string: &str,
        ) -> Result<Self, InvalidSettingsImportError> {
            let bytes = general_purpose::STANDARD
                .decode(string)
                .map_err(|_| InvalidSettingsImportError::InvalidBase64)?;
            let result = bincode_next::serde::decode_from_slice::<Self, _>(
                bytes.as_slice(),
                bincode_next::config::legacy(),
            )
            .map_err(|_| InvalidSettingsImportError::DeserialisationFailed)?
            .0;
            Ok(result)
        }
    }

    impl From<UserSettings> for crate::settings::UserSettings {
        fn from(val: UserSettings) -> crate::settings::UserSettings {
            crate::settings::UserSettings {
                zoom: val.zoom,
                centre: val.centre,
                iterations: val.iterations,
                julia_set: val.julia_set,
                smoothen: val.smoothen,
                internal_black: val.internal_black,
                initial_value: val.initial_value,
                escape_threshold: val.escape_threshold,
                shader_data: CustomShaderData {
                    equation: val.equation,
                    colour: val.colour,
                    ..Default::default()
                },
                ..Default::default()
            }
        }
    }
}

pub(in crate::settings) mod v0_5 {
    use crate::settings::{CustomShaderData, InvalidSettingsImportError};

    use base64::engine::general_purpose;
    use base64::Engine;

    #[derive(Clone, serde::Serialize, serde::Deserialize)]
    pub(in crate::settings) struct UserSettings {
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
        pub(in crate::settings) fn import_base64(
            string: &str,
        ) -> Result<Self, InvalidSettingsImportError> {
            let bytes = general_purpose::STANDARD
                .decode(string)
                .map_err(|_| InvalidSettingsImportError::InvalidBase64)?;
            let result = bincode_next::serde::decode_from_slice::<Self, _>(
                bytes.as_slice(),
                bincode_next::config::legacy(),
            )
            .map_err(|_| InvalidSettingsImportError::DeserialisationFailed)?
            .0;
            Ok(result)
        }
    }

    impl From<UserSettings> for crate::settings::UserSettings {
        fn from(val: UserSettings) -> crate::settings::UserSettings {
            crate::settings::UserSettings {
                zoom: val.zoom,
                centre: val.centre,
                iterations: val.iterations,
                julia_set: val.julia_set,
                smoothen: val.smoothen,
                internal_black: val.internal_black,
                initial_value: val.initial_value,
                escape_threshold: val.escape_threshold,
                initial_c: val.initial_c,
                shader_data: CustomShaderData {
                    equation: val.equation,
                    colour: val.colour,
                    ..Default::default()
                },
            }
        }
    }
}

pub(in crate::settings) mod v2_0 {
    use crate::settings::{CustomShaderData, InvalidSettingsImportError};

    use base64::engine::general_purpose;
    use base64::Engine;

    #[derive(Clone, serde::Serialize, serde::Deserialize)]
    pub(in crate::settings) struct UserSettings {
        zoom: f32,
        centre: [f32; 2],
        iterations: i32,
        equation: String,
        colour: String,
        julia_set: bool,
        smoothen: bool,
        internal_black: bool,
        initial_value: [f32; 2],
        escape_threshold: f32,
        initial_c: bool,
    }

    impl UserSettings {
        pub(in crate::settings) fn import_base64(
            string: &str,
        ) -> Result<Self, InvalidSettingsImportError> {
            let bytes = general_purpose::STANDARD
                .decode(string)
                .map_err(|_| InvalidSettingsImportError::InvalidBase64)?;
            let result = bincode_next::serde::decode_from_slice::<Self, _>(
                bytes.as_slice(),
                bincode_next::config::legacy(),
            )
            .map_err(|_| InvalidSettingsImportError::DeserialisationFailed)?
            .0;
            Ok(result)
        }
    }

    impl From<UserSettings> for crate::settings::UserSettings {
        fn from(val: UserSettings) -> crate::settings::UserSettings {
            crate::settings::UserSettings {
                zoom: val.zoom,
                centre: val.centre,
                iterations: val.iterations,
                julia_set: val.julia_set,
                smoothen: val.smoothen,
                internal_black: val.internal_black,
                initial_value: val.initial_value,
                escape_threshold: val.escape_threshold,
                initial_c: val.initial_c,
                shader_data: CustomShaderData {
                    equation: val.equation,
                    colour: val.colour,
                    ..Default::default()
                },
            }
        }
    }
}
