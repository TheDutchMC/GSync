//! # GSync
//! GSync is a tool to help you stay backed up. It does this by synchronizing the folders you want to Google Drive, while respecting .gitignore files
//!
//! ## Installation
//! You've got two options to install GSync
//!
//! 1. Preferred method: Via crates.io: `cargo install gsync`
//! 2. Via GitHub: [Releases](https://github.com/TheDutchMC/GSync/releases)
//!
//! ## Usage
//! 1. Create a project on [Google Deveopers](https://console.developers.google.com)
//! 2. Configure the OAuth2 consent screen and create OAuth2 credentials
//! 3. Enable the Google Drive API
//! 4. If you are planning to use a Team Drive/Shared Drive, run `gsync drives` to get the ID of the drive you want to sync to
//! 5. Configure GSync: `gsync config -i <GOOGLE APP ID> -s <GOOGLE APP SECRET> -f <INPUT FILES> -d <ID OF SHARED DRIVE>`. The `-d` parameter is optional
//! 6. Login: `gsync login`
//! 7. Sync away! `gsync sync`
//!
//! To update your configuration later, run `gsync config` again, you don't have to re-provide all options if you don't want to change them
//!
//! ## Licence
//! GSync is dual licenced under the MIT and Apache-2.0 licence, at your discretion


#![deny(deprecated)]
#![deny(clippy::panic)]

#![warn(rust_2018_idioms)]
#![warn(clippy::cargo)]
#![warn(clippy::decimal_literal_representation)]
#![warn(clippy::if_not_else)]
#![warn(clippy::large_digit_groups)]
#![warn(clippy::missing_docs_in_private_items)]
#![warn(clippy::missing_errors_doc)]
#![warn(clippy::needless_continue)]

#![allow(clippy::multiple_crate_versions)]

mod api;
mod env;
mod config;
mod login;
mod macros;
mod sync;

use clap::Arg;
use crate::env::Env;
use crate::config::Configuration;
use crate::api::GoogleError;

/// Type alias for Result
pub type Result<T> = std::result::Result<T, (Error, u32, &'static str)>;

/// Enum describing Errors which can often occur in Gsync
#[derive(Debug)]
pub enum Error {
    /// Error returned by the Google API
    GoogleError(GoogleError),

    /// Error resulting from a database operation
    DatabaseError(rusqlite::Error),

    /// Error resulting from a reqwest operation
    RequestError(reqwest::Error),

    /// An error which does not fit in any other category
    Other(String)
}

/// Version of the binary. Set in Cargo.toml
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let matches = clap::App::new("gsync")
        .version(VERSION)
        .author("Tobias de Bruijn <t.debruijn@array21.dev>")
        .about("Sync folders and files to Google Drive while respecting gitignore files")
        .subcommand(clap::SubCommand::with_name("config")
            .about("Configure GSync. Not all options have to be supplied, if you don't want to overwrite them. If this is the first time you're running the config command, you must provide all options.")
            .arg(Arg::with_name("client-id")
                .short("i")
                .long("id")
                .value_name("CLIENT_ID")
                .help("The Client ID provided by Google")
                .takes_value(true)
                .required(false))
            .arg(Arg::with_name("client-secret")
                .short("s")
                .long("secret")
                .value_name("CLIENT_SECRET")
                .help("The Client Secret provided by Google")
                .takes_value(true)
                .required(false))
            .arg(Arg::with_name("files")
                .short("f")
                .long("files")
                .value_name("FILES")
                .help("The files you want to sync, comma seperated String")
                .takes_value(true)
                .required(false))
            .arg(Arg::with_name("drive_id")
                .short("d")
                .long("drive")
                .value_name("ID")
                .help("The ID of the Team Drive to use, if you are not using a Team Drive leave this empty.")
                .takes_value(true)
                .required(false)))
        .subcommand(clap::SubCommand::with_name("show")
            .about("Show the current GSync configuration"))
        .subcommand(clap::SubCommand::with_name("login")
            .about("Login to Google"))
        .subcommand(clap::SubCommand::with_name("sync")
            .about("Start syncing the configured folders to Google Drive"))
        .subcommand(clap::SubCommand::with_name("drives")
            .about("Get a list of all shared drives and their IDs."))
        .get_matches();

    let empty_env = Env::empty();

    // Scoping this seperately because we want to drop conn when we're done, since we can only ever have 1 conn.
    {
        //Check if there are tables
        let conn = empty_env.get_conn().expect("Failed to create database connection. ");
        conn.execute("CREATE TABLE IF NOT EXISTS user (id TEXT PRIMARY KEY, refresh_token TEXT, access_token TEXT, expiry INTEGER)", rusqlite::named_params! {}).expect("Failed to create table 'users'");
        conn.execute("CREATE TABLE IF NOT EXISTS config (client_id TEXT, client_secret TEXT, input_files TEXT, drive_id TEXT)", rusqlite::named_params! {}).expect("Failed to create table 'config'");
    }

    // 'config' subcommand
    if let Some(matches) = matches.subcommand_matches("config") {
        let new_config = Configuration {
            client_id:      option_str_string(matches.value_of("client-id")),
            client_secret:  option_str_string(matches.value_of("client-secret")),
            input_files:    option_str_string(matches.value_of("files")),
            drive_id:       option_str_string(matches.value_of("drive_id"))
        };

        let current_config = handle_err!(Configuration::get_config(&empty_env));
        let config = Configuration::merge(new_config, current_config);
        match config.is_complete() {
            (true, _) => {},
            (false, str) => {
                eprintln!("Error: Configuration is incomplete; {}", str);
                std::process::exit(1);
            }
        }

        handle_err!(config.write(&empty_env));

        println!("Configuration updated!");
        std::process::exit(0);
    }

    // 'show' subcommand
    if matches.subcommand_matches("show").is_some() {
        let config = handle_err!(Configuration::get_config(&empty_env));

        if config.is_empty() {
            println!("GSync is unconfigured. Run 'gsync config -h` for more information on how to configure GSync'");
            std::process::exit(0);
        }

        println!("Current GSync configuration:");
        println!("Client ID: {}", option_unwrap_text(config.client_id));
        println!("Client Secret: {}", option_unwrap_text(config.client_secret));
        println!("Input Files: {}", option_unwrap_text(config.input_files));
        println!("Drive ID: {}", option_unwrap_text(config.drive_id));
        std::process::exit(0);
    }

    // 'login' subcommand
    if matches.subcommand_matches("login").is_some() {
        let config = handle_err!(Configuration::get_config(&empty_env));

        if config.is_empty() {
            println!("GSync is unconfigured. Run 'gsync config -h` for more information on how to configure GSync'");
            std::process::exit(0);
        }

        match config.is_complete() {
            (true, _) => {},
            (false, str) => {
                eprintln!("Error: Configuration is incomplete; {}", str);
                std::process::exit(1);
            }
        }

        // Safe to call unwrap because we've verified that the config is complete
        let env = Env::new(config.client_id.as_ref().unwrap(), config.client_secret.as_ref().unwrap(), config.drive_id.as_ref(), String::new());
        let login_data = handle_err!(crate::login::perform_oauth2_login(&env));

        println!("Info: Inserting tokens into database.");
        handle_err!(crate::login::db::save_to_database(&login_data, &env));
        println!("Info: Login successful!");
        std::process::exit(0);
    }

    // 'sync' subcommand
    if matches.subcommand_matches("sync").is_some() {
        let config = handle_err!(Configuration::get_config(&empty_env));

        if config.is_empty() {
            println!("GSync is unconfigured. Run 'gsync config -h` for more information on how to configure GSync'");
            std::process::exit(0);
        }

        match config.is_complete() {
            (true, _) => {},
            (false, str) => {
                eprintln!("Error: Configuration is incomplete; {}", str);
                std::process::exit(1);
            }
        }

        if !handle_err!(is_logged_in(&empty_env)) {
            eprintln!("Error: GSync isn't logged in with Google. Have you run `gsync login` yet?");
            std::process::exit(1);
        }

        // Safe to call unwrap because we verified the config is complete above
        let mut env = Env::new(config.client_id.as_ref().unwrap(), config.client_secret.as_ref().unwrap(), config.drive_id.as_ref(), String::new());

        println!("Info: Querying Drive for root folder");
        let list = handle_err!(crate::api::drive::list_files(&env, Some("name = 'GSync' and mimeType = 'application/vnd.google-apps.folder' and trashed = false"), config.drive_id.as_deref()));

        let root_folder_id = if list.is_empty() {
            println!("Info: Root folder doesn't exist. Creating one now.");
            match &env.drive_id {
                Some(drive_id) => handle_err!(crate::api::drive::create_folder(&env, "GSync", drive_id)),
                None => handle_err!(crate::api::drive::create_folder(&env, "GSync", "root"))
            }
        } else {
            println!("Info: Root folder exists.");
            list.get(0).unwrap().id.clone()
        };

        env.root_folder = root_folder_id;

        handle_err!(crate::sync::sync(&config, &env));
        std::process::exit(0);
    }

    if matches.subcommand_matches("drives").is_some() {
        let config = handle_err!(Configuration::get_config(&empty_env));

        if config.is_empty() {
            println!("GSync is unconfigured. Run 'gsync config -h` for more information on how to configure GSync'");
            std::process::exit(0);
        }

        match config.is_complete() {
            (true, _) => {},
            (false, str) => {
                eprintln!("Error: Configuration is incomplete; {}", str);
                std::process::exit(1);
            }
        }

        if !handle_err!(is_logged_in(&empty_env)) {
            eprintln!("Error: GSync isn't logged in with Google. Have you run `gsync login` yet?");
            std::process::exit(1);
        }

        let env = Env::new(config.client_id.as_ref().unwrap(), config.client_secret.as_ref().unwrap(), config.drive_id.as_ref(), String::new());
        let shared_drives = handle_err!(crate::api::drive::get_shared_drives(&env));
        for drive in shared_drives {
            println!("Shared drive '{}' with identifier '{}'", &drive.name, &drive.id);
        }

        std::process::exit(0);
    }

    println!("No command specified. Run 'gsync -h' for available commands.");
}

/// Convert a Option<&str> to an Option<String>
fn option_str_string(i: Option<&str>) -> Option<String> {
    i.map(|i| i.to_string())
}

/// Unwrap an Option<String> to a String. If the input is None, you'll get back the literal `None`
fn option_unwrap_text(i: Option<String>) -> String {
    match i {
        Some(i) => i,
        None => "None".to_string()
    }
}

/// Check if a user is logged in
///
/// # Errors
/// - When a database operation fails
fn is_logged_in(env: &Env) -> Result<bool> {
    let conn = unwrap_db_err!(env.get_conn());
    let mut stmt = unwrap_db_err!(conn.prepare("SELECT * FROM user"));
    let mut result = unwrap_db_err!(stmt.query(rusqlite::named_params! {}));

    let mut is_logged_in = false;
    while let Ok(Some(_)) = result.next() {
        is_logged_in = true;
    }

    Ok(is_logged_in)
}