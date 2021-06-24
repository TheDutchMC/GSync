
mod api;
mod env;
mod config;
mod login;

use clap::Arg;
use crate::env::Env;
use crate::config::Configuration;

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
                .required(false)))
        .subcommand(clap::SubCommand::with_name("show")
            .about("Show the current Syncer configuration"))
        .subcommand(clap::SubCommand::with_name("login")
            .about("Login to Google"))
        .subcommand(clap::SubCommand::with_name("sync")
            .about("Start syncing the configured folders to Google Drive"))
        .get_matches();

    let empty_env = Env::empty();

    //Check if there are tables
    let conn = empty_env.get_conn().expect("Failed to create database connection. ");
    conn.execute("CREATE TABLE IF NOT EXISTS user (id TEXT PRIMARY KEY, refresh_token TEXT, access_token TEXT, expires_in INTEGER)", rusqlite::named_params! {}).expect("Failed to create table 'users'");
    conn.execute("CREATE TABLE IF NOT EXISTS config (client_id TEXT, client_secret TEXT, input_files TEXT)", rusqlite::named_params! {}).expect("Failed to create table 'config'");
    conn.execute("CREATE TABLE IF NOT EXISTS files (id TEXT PRIMARY KEY, path TEXT, hash TEXT)", rusqlite::named_params! {}).expect("Failed to create table 'files'");

    // 'config' subcommand
    if let Some(matches) = matches.subcommand_matches("config") {
        let new_config = Configuration {
            client_id: option_str_string(matches.value_of("client-id")),
            client_secret: option_str_string(matches.value_of("client-secret")),
            input_files: option_str_string(matches.value_of("files"))
        };

        let current_config = match Configuration::get_config(&empty_env) {
            Ok(cc) => cc,
            Err(e) => {
                eprintln!("Error: Failed to query current configuration: {:?}", e);
                std::process::exit(1);
            }
        };

        let config = Configuration::merge(new_config, current_config);
        println!("{:?}", &config);
        match config.is_complete() {
            (true, _) => {},
            (false, str) => {
                eprintln!("Error: Configuration is incomplete; {}", str);
                std::process::exit(1);
            }
        }

        match config.write(&empty_env) {
            Ok(_) => {},
            Err(e) => {
                eprintln!("Error: Failed to write new configuration: {:?}", e);
                std::process::exit(1);
            }
        };

        println!("Configuration updated!");
        std::process::exit(0);
    }

    // 'show' subcommand
    if let Some(_) = matches.subcommand_matches("show") {
        let config = match Configuration::get_config(&empty_env) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error: Failed to query configuration: {:?}", e);
                std::process::exit(1);
            }
        };

        if config.is_empty() {
            println!("Syncer is unconfigured. Run 'syncer config -h` for more information on how to configure Syncer'");
            std::process::exit(0);
        }

        println!("Current Syncer configuration:");
        println!("Client ID: {}", option_unwrap_text(config.client_id));
        println!("Client Secret: {}", option_unwrap_text(config.client_secret));
        println!("Input Files: {}", option_unwrap_text(config.input_files));
        std::process::exit(0);
    }

    // 'login' subcommand
    if let Some(_) = matches.subcommand_matches("login") {
        let config = match Configuration::get_config(&empty_env) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error: Failed to query configuration: {:?}", e);
                std::process::exit(1);
            }
        };

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

        let env = Env::new(config.client_id.as_ref().unwrap(), config.client_secret.as_ref().unwrap());
        let login_data = match crate::login::perform_oauth2_login(&env) {
            Ok(ld) => ld,
            Err(e) => {
                eprintln!("Error: OAuth2 Login Flow failed: {}", e);
                std::process::exit(1);
            }
        };

        println!("Info: Inserting tokens into database.");

        match crate::login::db::UserLogin::save_to_database(&login_data.access_token, login_data.expires_in, &login_data.refresh_token, &env) {
            Ok(_) => {},
            Err(e) => {
                eprintln!("Error: Failed to insert login credentials into database: {}", e);
                std::process::exit(1);
            }
        }

        println!("Info: Login successful!");
        std::process::exit(0);
    }

    // 'sync' subcommand
    if let Some(_) = matches.subcommand_matches("sync") {
        let config = match Configuration::get_config(&empty_env) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error: Failed to query configuration: {:?}", e);
                std::process::exit(1);
            }
        };

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

        let _env = Env::new(config.client_id.as_ref().unwrap(), config.client_secret.as_ref().unwrap());
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