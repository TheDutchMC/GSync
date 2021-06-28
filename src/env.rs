//! Module describing user-configurable and program-fetched variables needed for proper operation of GSync

use cfg_if::cfg_if;

/// Struct describing the environment of GSync
#[derive(Debug, Clone)]
pub struct Env {
    /// Database path
    pub db:             String,

    /// Google client ID
    pub client_id:      String,

    /// Google Client Secret
    pub client_secret:  String,

    /// If using a Team Drive/Shared Drive, the ID of that drive
    pub drive_id:       Option<String>,

    /// The ID of the root folder ('GSync')
    pub root_folder:    String
}

#[cfg(unix)]
/// Unix path to the gsync home folder
const DB_PATH: &str = "%home%/.gsync/";

#[cfg(windows)]
/// Windows path to the gsync home folder
const DB_PATH: &str = r#"%appdata%\gsync\"#;

impl Env {
    /// Create a new instance of Env
    pub fn new<A, B, C, D>(id: A, secret: B, drive_id: Option<C>, root_folder: D) -> Self
    where A: AsRef<str>, B: AsRef<str>, C: AsRef<str>, D: AsRef<str> {
        let db = get_db_path();
        if !std::path::Path::new(&db).exists() {
            #[allow(clippy::panic)]
            std::fs::create_dir_all(std::path::Path::new(&db)).unwrap_or_else(|f| panic!("Failed to create database folder at {}: {:?} ", &db, f));
        }

        Self {
            db,
            client_secret:  secret.as_ref().to_string(),
            client_id:      id.as_ref().to_string(),
            drive_id:       drive_id.map(|id| id.as_ref().to_string()),
            root_folder:    root_folder.as_ref().to_string()
        }
    }

    /// Create an empty instance of Env
    pub fn empty() -> Self {

        let db = get_db_path();
        if !std::path::Path::new(&db).exists() {
            #[allow(clippy::panic)]
            std::fs::create_dir_all(std::path::Path::new(&db)).unwrap_or_else(|f| panic!("Failed to create database folder at {}: {:?} ", &db, f));
        }

        Self {
            db,
            client_id:      String::new(),
            client_secret:  String::new(),
            drive_id:       None,
            root_folder:    String::new()
        }
    }

    /// Get a connection to the database
    pub fn get_conn(&self) -> Result<rusqlite::Connection, rusqlite::Error> {
        let mut path = std::path::PathBuf::from(&self.db);
        path.push("data.db3");

        rusqlite::Connection::open(path.as_path())
    }
}

cfg_if! {
    if #[cfg(unix)] {
        /// Get the database path
        fn get_db_path() -> String {
            DB_PATH.replace("%home%", &std::env::var("HOME").unwrap())
        }
    } else if #[cfg(windows)] {
        /// Get the database path
        fn get_db_path() -> String {
            DB_PATH.replace("%appdata%", &std::env::var("appdata").unwrap())
        }
    } else {
        /// Get the database path
        #[allow(clippy::panic)]
        fn get_db_path() -> String {
            panic!("Unsupported platform!");
        }
    }
}
