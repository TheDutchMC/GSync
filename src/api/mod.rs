pub mod drive;
pub mod oauth;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct GoogleResponse<T> {
    #[serde(flatten)]
    data:   Option<T>,
    error:  Option<GoogleError>
}

#[derive(Deserialize, Debug)]
pub struct GoogleError {
    code:       i16,
    message:    String,
    errors:     Vec<ErrorData>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ErrorData {
    domain:         String,
    reason:         String,
    message:        String,
    location_type:  String,
    location:       String
}