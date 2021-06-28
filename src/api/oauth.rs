//! Google OAuth2 API

use crate::env::Env;
use serde::{Deserialize, Serialize};

use crate::{Result, unwrap_req_err, unwrap_db_err, unwrap_google_err};
use crate::api::GoogleResponse;

/// Login Data
pub struct LoginData {
    /// Refresh token
    pub refresh_token:  Option<String>,

    /// Access token
    pub access_token:   String,

    /// Seconds until the refresh token expires
    pub expires_in:     i64
}

/// Struct describing the request to exchange an access code for an access token
#[derive(Serialize)]
struct ExchangeAccessTokenRequest<'a> {
    /// The application's client ID
    client_id:          &'a str,

    /// The application's client secret
    client_secret:      &'a str,

    /// The access code
    code:               &'a str,

    /// The verifier halve of the code challenge
    code_verifier:      &'a str,

    /// The grant type
    grant_type:         &'static str,

    /// The original redirect URI
    redirect_uri:       &'a str
}

/// Struct describing the response to an access token exchange request
#[derive(Deserialize)]
struct ExchangeAccessTokenResponse {
    /// The access token
    access_token:   String,

    /// Seconds until the access token expires
    expires_in:     i64,

    /// The refresh token used to refresh the access token
    refresh_token:  String,
}

/// Struct describing an authentication request
#[derive(Serialize)]
struct AuthenticationRequest<'a> {
    /// Application's client ID
    client_id:              &'a str,

    /// The original redirect URI
    redirect_uri:           &'a str,

    /// The response type
    response_type:          &'static str,

    /// The scopes requested
    scope:                  &'static str,

    /// The challenge halve of the code challenge
    code_challenge:         &'a str,

    /// The method of code challenge
    code_challenge_method:  &'static str,

    /// State parameter
    state:                  &'a str,
}

/// Struct describing the request to refresh an access token
#[derive(Serialize)]
struct RefreshTokenRequest<'a> {
    /// Application's client ID
    client_id:      &'a str,

    /// Application's Client Secret
    client_secret:  &'a str,

    /// The type of grant
    grant_type:     &'static str,

    /// The refresh token
    refresh_token:  &'a str
}


/// Struct describing the response for refreshing an access token
#[derive(Deserialize)]
struct RefreshTokenResponse {
    /// The new access token
    access_token:   String,

    /// Seconds until the token expires
    expires_in:     i64,
}

/// Create an authentication URL used for step 1 in the OAuth2 flow
pub fn create_authentication_uri(env: &Env, code_challenge: &str, state: &str, redirect_uri: &str) -> String {
    let auth_request = AuthenticationRequest {
        client_id:              &env.client_id,
        redirect_uri,
        response_type:          "code",
        scope:                  "https://www.googleapis.com/auth/drive",
        code_challenge:         &code_challenge,
        code_challenge_method:  "S256",
        state:                  &state
    };

    let qstring = serde_qs::to_string(&auth_request).unwrap();
    format!("https://accounts.google.com/o/oauth2/v2/auth?{}", qstring)
}


/// Exchange an access code for an access token
///
/// ## Errors
/// - Google API error
/// - Reqwest error
pub fn exchange_access_token(env: &Env, access_token: &str, code_verifier: &str, redirect_uri: &str) -> Result<LoginData> {

    //We can now exchange this token for a refresh_token and the likes
    let exchange_request = ExchangeAccessTokenRequest {
        client_id: &env.client_id,
        client_secret: &env.client_secret,
        code: access_token,
        code_verifier,
        grant_type: "authorization_code",
        redirect_uri
    };

    // Send a request to Google to exchange the code for the necessary codes
    let response = unwrap_req_err!(reqwest::blocking::Client::new().post("https://oauth2.googleapis.com/token")
        .body(serde_json::to_string(&exchange_request).unwrap())
        .send());

    // Deserialize from JSON
    let exchange_response: GoogleResponse<ExchangeAccessTokenResponse> = unwrap_req_err!(response.json());
    let token_response = unwrap_google_err!(exchange_response);

    Ok(LoginData {
        access_token: token_response.access_token,
        refresh_token: Some(token_response.refresh_token),
        expires_in: token_response.expires_in
    })
}

/// Get an access token
///
/// ## Errors
/// - When a database error occurs
/// - When the Google API returns an error
/// - When reqwest returns an error
pub fn get_access_token(env: &Env) -> Result<String> {
    let conn = unwrap_db_err!(env.get_conn());
    let mut stmt = unwrap_db_err!(conn.prepare("SELECT access_token, refresh_token, expiry FROM user"));
    let mut result = unwrap_db_err!(stmt.query(rusqlite::named_params! {}));

    if let Ok(Some(row)) = result.next() {
        let access_token = unwrap_db_err!(row.get::<&str, String>("access_token"));
        let refresh_token = unwrap_db_err!(row.get::<&str, String>("refresh_token"));
        let expiry = unwrap_db_err!(row.get::<&str, i64>("expiry"));

        if chrono::Utc::now().timestamp() > (expiry - 60) {
            // We need to manually drop these to avoid having two open connections at the same time
            // Since sqlite won't allow that
            drop(result);
            drop(stmt);
            drop(conn);
            let new_token = refresh_access_token(env, &refresh_token)?;
            crate::login::db::save_to_database(&new_token, env)?;

            return Ok(new_token.access_token);
        }

        return Ok(access_token)
    }

    Ok(String::default())

}

/// Refresh an OAuth2 access token using a refresh token
///
/// ## Errors
/// - When the Google API returns an error
/// - When reqwest returns an error
fn refresh_access_token(env: &Env, refresh_token: &str) -> Result<LoginData> {
    let request_body = RefreshTokenRequest {
        client_id:      &env.client_id,
        client_secret:  &env.client_secret,
        grant_type:     "refresh_token",
        refresh_token
    };

    //Safe to unwrap() because we know the struct can be translated to valid json
    let body = serde_json::to_string(&request_body).unwrap();
    let request = unwrap_req_err!(reqwest::blocking::Client::new().post("https://oauth2.googleapis.com/token")
        .body(body)
        .send());

    let response_payload: GoogleResponse<RefreshTokenResponse> = unwrap_req_err!(request.json());
    let payload = unwrap_google_err!(response_payload);

    Ok(LoginData {
        access_token: payload.access_token,
        expires_in: payload.expires_in,
        refresh_token: None
    })
}