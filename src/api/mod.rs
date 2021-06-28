//! Common Google API types

pub mod drive;
pub mod oauth;

use serde::Deserialize;

/// Struct describing a generic response from a Google API
#[derive(Deserialize, Debug)]
pub struct GoogleResponse<T> {
    #[serde(flatten)]
    /// The data returned by Google, if there was no error
    pub data:   Option<T>,

    /// The error returned by Google, if there was an error
    pub error:  Option<GoogleError>
}

/// Struct describing an error response from a Google API
#[derive(Deserialize, Debug)]
pub struct GoogleError {
    /// The error code
    pub code:       i16,

    /// The error message
    pub message:    String,

    /// Specific details around the error(s)
    pub errors:     Vec<ErrorData>
}

/// Struct describing a specific Error returned from a Google API
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ErrorData {
    /// The domain in which the error occurred
    pub domain:         String,

    /// The reason why the error occured
    pub reason:         String,

    /// The error message
    pub message:        String,

    /// The location type at which the error occurred
    pub location_type:  Option<String>,

    /// The location at which the error occurred
    pub location:       Option<String>
}