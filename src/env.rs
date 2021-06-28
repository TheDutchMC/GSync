use cfg_if::cfg_if;

#[derive(Debug, Clone)]
pub struct Env {
    pub db:             String,
    pub client_id:      String,
    pub client_secret:  String,
    pub drive_id:       Option<String>,
    pub root_folder:    String
}

#[cfg(unix)]
const DB_PATH: &str = "%home%/.gsync/";

#[cfg(windows)]
const DB_PATH: &str = r#"%appdata%\gsync\"#;

impl Env {
    pub fn new<A, B, C, D>(id: A, secret: B, drive_id: Option<C>, root_folder: D) -> Self
    where A: AsRef<str>, B: AsRef<str>, C: AsRef<str>, D: AsRef<str> {
        let db = get_db_path();
        if !std::path::Path::new(&db).exists() {
            std::fs::create_dir_all(std::path::Path::new(&db)).expect(&format!("Failed to create database folder at {}. ", &db));
        }

        Self {
            db,
            client_secret:  secret.as_ref().to_string(),
            client_id:      id.as_ref().to_string(),
            drive_id:       match drive_id {
                                Some(id) => Some(id.as_ref().to_string()),
                                None => None
                            },
            root_folder:    root_folder.as_ref().to_string()
        }
    }

    pub fn empty() -> Self {
        let db = get_db_path();
        if !std::path::Path::new(&db).exists() {
            std::fs::create_dir_all(std::path::Path::new(&db)).expect(&format!("Failed to create database folder at {}. ", &db));
        }

        Self {
            db,
            client_id:      String::new(),
            client_secret:  String::new(),
            drive_id:       None,
            root_folder:    String::new()
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
