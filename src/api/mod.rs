pub mod drive;
pub mod oauth;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct GoogleError {
    code:       i16,
    message:    String,
    errors:     Vec<SingleGoogleError>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SingleGoogleError {
    domain:         String,
    reason:         String,
    message:        String,
    location_type:  String,
    location:       String
}