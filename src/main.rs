
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

pub type Result<T> = std::result::Result<T, (Error, u32, &'static str)>;

#[derive(Debug)]
pub enum Error {
    GoogleError(GoogleError),
    DatabaseError(rusqlite::Error),
    RequestError(reqwest::Error),
    Other(String)
}

fn main() {
    let matches = clap::App::new("Syncer")
        .version("0.1.0")
        .author("Tobias de Bruijn <t.debruijn@array21.dev>")
        .about("Sync folders and files to Google Drive while respecting gitignore files")
        .subcommand(clap::SubCommand::with_name("config")
            .about("Configure Syncer. Not all options have to be supplied, if you don't want to overwrite them. If this is the first time you're running the config command, you must provide all options.")
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
            .about("Show the current Syncer configuration"))
        .subcommand(clap::SubCommand::with_name("login")
            .about("Login to Google"))
        .subcommand(clap::SubCommand::with_name("sync")
            .about("Start syncing the configured folders to Google Drive"))
        .get_matches();

    let empty_env = Env::empty();

    // Scoping this seperately because we want to drop conn when we're done, since we can only ever have 1 conn.
    {
        //Check if there are tables
        let conn = empty_env.get_conn().expect("Failed to create database connection. ");
        conn.execute("CREATE TABLE IF NOT EXISTS user (id TEXT PRIMARY KEY, refresh_token TEXT, access_token TEXT, expiry INTEGER)", rusqlite::named_params! {}).expect("Failed to create table 'users'");
        conn.execute("CREATE TABLE IF NOT EXISTS config (client_id TEXT, client_secret TEXT, input_files TEXT, drive_id TEXT)", rusqlite::named_params! {}).expect("Failed to create table 'config'");
        conn.execute("CREATE TABLE IF NOT EXISTS files (id TEXT PRIMARY KEY, path TEXT, modification_time INTEGER, sync_include INTEGER)", rusqlite::named_params! {}).expect("Failed to create table 'files'");
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
    if let Some(_) = matches.subcommand_matches("show") {
        let config = handle_err!(Configuration::get_config(&empty_env));

        if config.is_empty() {
            println!("Syncer is unconfigured. Run 'syncer config -h` for more information on how to configure Syncer'");
            std::process::exit(0);
        }

        println!("Current Syncer configuration:");
        println!("Client ID: {}", option_unwrap_text(config.client_id));
        println!("Client Secret: {}", option_unwrap_text(config.client_secret));
        println!("Input Files: {}", option_unwrap_text(config.input_files));
        println!("Drive ID: {}", option_unwrap_text(config.drive_id));
        std::process::exit(0);
    }

    // 'login' subcommand
    if let Some(_) = matches.subcommand_matches("login") {
        let config = handle_err!(Configuration::get_config(&empty_env));

        if config.is_empty() {
            println!("Syncer is unconfigured. Run 'syncer config -h` for more information on how to configure Syncer'");
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
        handle_err!(crate::login::db::UserLogin::save_to_database(&login_data, &env));
        println!("Info: Login successful!");
        std::process::exit(0);
    }

    // 'sync' subcommand
    if let Some(_) = matches.subcommand_matches("sync") {
        let config = handle_err!(Configuration::get_config(&empty_env));

        if config.is_empty() {
            println!("Syncer is unconfigured. Run 'syncer config -h` for more information on how to configure Syncer'");
            std::process::exit(0);
        }

        match config.is_complete() {
            (true, _) => {},
            (false, str) => {
                eprintln!("Error: Configuration is incomplete; {}", str);
                std::process::exit(1);
            }
        }

        // Safe to call unwrap because we verified the config is complete above
        let mut env = Env::new(config.client_id.as_ref().unwrap(), config.client_secret.as_ref().unwrap(), config.drive_id.as_ref(), String::new());
        let access_token = handle_err!(crate::api::oauth::get_access_token(&env));

        let list = handle_err!(crate::api::drive::list_files(&access_token, Some("name = 'Syncer' and mimeType = 'application/vnd.google-apps.folder'"), config.drive_id.as_deref()));
        println!("{:#?}", &list);

        let root_folder_id = if list.len() == 0 {
            println!("Info: Root folder doesn't exist. Creating one now.");
            handle_err!(crate::api::drive::create_folder(&access_token, "Syncer", "root"))
        } else {
            println!("Info: Root folder exists.");
            list.get(0).unwrap().id.clone()
        };

        env.root_folder = root_folder_id;

        handle_err!(crate::sync::sync(&config, &env));
        std::process::exit(0);
    }

    println!("No command specified. Run 'syncer -h' for available commands.");
}

fn option_str_string(i: Option<&str>) -> Option<String> {
    match i {
        Some(i) => Some(i.to_string()),
        None => None
    }
}

fn option_unwrap_text(i: Option<String>) -> String {
    match i {
        Some(i) => i,
        None => "None".to_string()
    }
}