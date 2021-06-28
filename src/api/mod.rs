pub mod drive;
pub mod oauth;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct GoogleResponse<T> {
    #[serde(flatten)]
    pub data:   Option<T>,
    pub error:  Option<GoogleError>
}

#[derive(Deserialize, Debug)]
pub struct GoogleError {
    pub code:       i16,
    pub message:    String,
    pub errors:     Vec<ErrorData>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ErrorData {
    pub domain:         String,
    pub reason:         String,
    pub message:        String,
    pub location_type:  Option<String>,
    pub location:       Option<String>
}