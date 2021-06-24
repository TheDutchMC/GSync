use cfg_if::cfg_if;

#[derive(Debug, Clone)]
pub struct Env {
    pub db:             String,
    pub client_id:      String,
    pub client_secret:  String
}

#[cfg(unix)]
const DB_PATH: &str = "%home%/.syncer/";

#[cfg(windows)]
const DB_PATH: &str = r#"%appdata%\syncer\"#;

impl Env {
    pub fn new(id: &str, secret: &str) -> Self {
        let db = get_db_path();
        if !std::path::Path::new(&db).exists() {
            std::fs::create_dir_all(std::path::Path::new(&db)).expect(&format!("Failed to create database folder at {}. ", &db));
        }

        Self {
            db,
            client_secret: secret.to_string(),
            client_id: id.to_string()
        }
    }

    pub fn empty() -> Self {
        let db = get_db_path();
        if !std::path::Path::new(&db).exists() {
            std::fs::create_dir_all(std::path::Path::new(&db)).expect(&format!("Failed to create database folder at {}. ", &db));
        }

        Self {
            db,
            client_id: String::new(),
            client_secret: String::new()
        }
    }

    pub fn get_conn(&self) -> Result<rusqlite::Connection, rusqlite::Error> {
        let mut path = std::path::PathBuf::from(&self.db);
        path.push("data.db3");

        rusqlite::Connection::open(path.as_path())
    }
}

cfg_if! {
    if #[cfg(unix)] {
        fn get_db_path() -> String {
            DB_PATH.replace("%home%", &std::env::var("HOME").unwrap())
        }
    } else if #[cfg(windows)] {
        fn get_db_path() -> String {
            DB_PATH.replace("%appdata%", &std::env::var("appdata").unwrap())
        }
    } else {
        fn get_db_path() -> String {
            panic!("Unsupported platform!");
        }
    }
}
