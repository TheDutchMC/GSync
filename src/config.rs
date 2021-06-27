use crate::env::Env;
use rusqlite::named_params;
use crate::{Result, unwrap_db_err, Error};

#[derive(Debug)]
pub struct Configuration {
    pub client_id:      Option<String>,
    pub client_secret:  Option<String>,
    pub input_files:    Option<String>
}

impl Configuration {

    pub fn is_empty(&self) -> bool {
        self.input_files.is_none() && self.client_id.is_none() && self.client_secret.is_none()
    }

    pub fn empty() -> Self {
        Self {
            client_id: None,
            client_secret: None,
            input_files: None
        }
    }

    pub fn is_complete(&self) -> (bool, &str) {
        if self.client_id.is_none() {
            (false, "'client_id' is empty")
        } else if self.client_secret.is_none() {
            (false, "'client_secret' is empty")
        } else if self.input_files.is_none() {
            (false, "'input_files' is empty")
        } else {
            (true, "")
        }
    }

    pub fn merge(a: Self, b: Self) -> Self {
        let mut output = Self::empty();
        match a.client_id {
            Some(s) => output.client_id = Some(s),
            None => output.client_id = b.client_id
        };

        match a.client_secret {
            Some(s) => output.client_secret = Some(s),
            None => output.client_secret = b.client_secret
        };

        match a.input_files {
            Some(s) => output.input_files = Some(s),
            None => output.input_files = b.input_files
        };

        output
    }

    pub fn get_config(env: &Env) -> Result<Self> {
        let conn = unwrap_db_err!(env.get_conn());

        let mut stmt = unwrap_db_err!(conn.prepare("SELECT * FROM config"));
        let mut result = unwrap_db_err!(stmt.query(named_params! {}));

        match result.next() {
            Ok(Some(row)) => {
                let client_id = unwrap_db_err!(row.get::<&str, Option<String>>("client_id"));
                let client_secret = unwrap_db_err!(row.get::<&str, Option<String>>("client_secret"));
                let input_files = unwrap_db_err!(row.get::<&str, Option<String>>("input_files"));

                Ok(Self { client_id, client_secret, input_files})
            },
            Ok(None) => Ok(Self::empty()),
            Err(e) => Err(Error::DatabaseError(e))
        }
    }

    pub fn write(&self, env: &Env) -> Result<()> {
        let conn = unwrap_db_err!(env.get_conn());

        unwrap_db_err!(conn.execute("DELETE FROM config", named_params! {}));

        unwrap_db_err!(conn.execute("INSERT INTO config (client_id, client_secret, input_files) VALUES (:client_id, :client_secret, :input_files)", named_params! {
            ":client_id": &self.client_id,
            ":client_secret": &self.client_secret,
            ":input_files": &self.input_files
        }));

        Ok(())
    }
}

