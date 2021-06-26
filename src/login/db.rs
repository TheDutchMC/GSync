use crate::env::Env;
use rusqlite::named_params;
use crate::api::oauth::LoginData;
use crate::unwrap_str;

pub struct UserLogin {
    _access_token: String,
    _expires_in:   i64
}

impl UserLogin {
    pub fn save_to_database(login_data: &LoginData, env: &Env) -> Result<(), String> {
        let conn = unwrap_str!(env.get_conn());
        unwrap_str!(conn.execute("DELETE FROM user", named_params! {}));

        let expiry_time = chrono::Utc::now().timestamp() + login_data.expires_in;
        unwrap_str!(if login_data.refresh_token.is_some() {
            conn.execute("INSERT INTO user (refresh_token, access_token, expiry) VALUES (:refresh_token, :access_token, :expiry)", named_params! {
                ":refresh_token": &login_data.refresh_token,
                ":access_token": &login_data.access_token,
                ":expiry": expiry_time
            })
        } else {
            conn.execute("UPDATE user SET access_token = :access_token, expiry = :expiry", named_params! {
                ":access_token": &login_data.access_token,
                ":expiry": expiry_time
            })
        });

        Ok(())
    }
}