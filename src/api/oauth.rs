use crate::env::Env;
use serde::{Deserialize, Serialize};
use crate::unwrap_str;

pub struct LoginData {
    pub refresh_token:  Option<String>,
    pub access_token:   String,
    pub expires_in:     i64
}

#[derive(Serialize)]
struct ExchangeAccessTokenRequest<'a> {
    client_id:          &'a str,
    client_secret:      &'a str,
    code:               &'a str,
    code_verifier:      &'a str,
    grant_type:         &'static str,
    redirect_uri:       &'a str
}

#[derive(Deserialize)]
struct ExchangeAccessTokenResponse {
    access_token:   String,
    expires_in:     i64,
    refresh_token:  String,
}

#[derive(Serialize)]
struct AuthenticationRequest<'a> {
    client_id:              &'a str,
    redirect_uri:           &'a str,
    response_type:          &'static str,
    scope:                  &'static str,
    code_challenge:         &'a str,
    code_challenge_method:  &'static str,
    state:                  &'a str,
}

#[derive(Serialize)]
struct RefreshTokenRequest<'a> {
    client_id:      &'a str,
    client_secret:  &'a str,
    grant_type:     &'static str,
    refresh_token:  &'a str
}

#[derive(Deserialize)]
struct RefreshTokenResponse {
    access_token:   String,
    expires_in:     i64,
}

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


pub fn exchange_access_token(env: &Env, access_token: &str, code_verifier: &str, redirect_uri: &str) -> Result<LoginData, String> {

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
    let response = unwrap_str!(reqwest::blocking::Client::new().post("https://oauth2.googleapis.com/token")
        .body(serde_json::to_string(&exchange_request).unwrap())
        .send());

    // Deserialize from JSON
    let exchange_response: ExchangeAccessTokenResponse = unwrap_str!(response.json());

    Ok(LoginData {
        access_token: exchange_response.access_token,
        refresh_token: Some(exchange_response.refresh_token),
        expires_in: exchange_response.expires_in
    })
}

pub fn get_access_token(env: &Env) -> Result<String, String> {
    let conn = unwrap_str!(env.get_conn());
    let mut stmt = unwrap_str!(conn.prepare("SELECT access_token, refresh_token, expiry FROM user"));
    let mut result = unwrap_str!(stmt.query(rusqlite::named_params! {}));

    while let Ok(Some(row)) = result.next() {
        let access_token = unwrap_str!(row.get::<&str, String>("access_token"));
        let refresh_token = unwrap_str!(row.get::<&str, String>("refresh_token"));
        let expiry = unwrap_str!(row.get::<&str, i64>("expiry"));

        if chrono::Utc::now().timestamp() > (expiry - 60) {
            // We need to manually drop these to avoid having two open connections at the same time
            // Since sqlite won't allow that
            drop(result);
            drop(stmt);
            drop(conn);
            let new_token = refresh_access_token(env, &refresh_token)?;
            crate::login::db::UserLogin::save_to_database(&new_token, env)?;

            return Ok(new_token.access_token);
        }

        return Ok(access_token)
    }

    Ok(String::default())

}

fn refresh_access_token(env: &Env, refresh_token: &str) -> Result<LoginData, String> {
    let request_body = RefreshTokenRequest {
        client_id:      &env.client_id,
        client_secret:  &env.client_secret,
        grant_type:     "refresh_token",
        refresh_token
    };

    //Safe to unwrap() because we know the struct can be translated to valid json
    let body = serde_json::to_string(&request_body).unwrap();
    let request = unwrap_str!(reqwest::blocking::Client::new().post("https://oauth2.googleapis.com/token")
        .body(body)
        .send());

    let response_payload: RefreshTokenResponse = unwrap_str!(request.json());

    Ok(LoginData {
        access_token: response_payload.access_token,
        expires_in: response_payload.expires_in,
        refresh_token: None
    })
}