use crate::env::Env;
use rusqlite::named_params;

pub struct UserLogin {
    _access_token: String,
    _expires_in:   i64
}

impl UserLogin {
    pub fn save_to_database(access_token: &str, expires_in: i64, refresh_token: &str, env: &Env) -> Result<(), String> {
        let conn = match env.get_conn() {
            Ok(c) => c,
            Err(e) => return Err(e.to_string())
        };

        match conn.execute("DELETE FROM user", named_params! {}) {
            Ok(_) => {},
            Err(e) => return Err(e.to_string())
        }

        match conn.execute("INSERT INTO user (refresh_token, access_token, expires_in) VALUES (:refresh_token, :access_token, :expires_in)", named_params! {
            ":refresh_token": refresh_token,
            ":access_token": access_token,
            ":expires_in": expires_in
        }) {
            Ok(_) => {},
            Err(e) => return Err(e.to_string())
        }

        Ok(())
    }
}