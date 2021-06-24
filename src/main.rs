
mod api;
mod env;

use clap::Arg;
use crate::env::Env;
use rusqlite::named_params;

struct Configuration {
    client_id:      Option<String>,
    client_secret:  Option<String>,
    input_files:    Option<String>
}

impl Configuration {
    fn is_valid(&self) -> (bool, &str) {
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
                .required(false)))
        .subcommand(clap::SubCommand::with_name("show")
            .about("Show the current Syncer configuration"))
        .get_matches();

    let empty_env = Env::empty();

    //Check if there are tables
    let conn = empty_env.get_conn().expect("Failed to create database connection. ");
    conn.execute("CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, refresh_token TEXT)", named_params! {}).expect("Failed to create table 'users'");
    conn.execute("CREATE TABLE IF NOT EXISTS config (client_id TEXT, client_secret TEXT, input_files TEXT)", named_params! {}).expect("Failed to create table 'config'");
    conn.execute("CREATE TABLE IF NOT EXISTS files (id TEXT PRIMARY KEY, path TEXT, hash TEXT)", named_params! {}).expect("Failed to create table 'files'");

    if let Some(matches) = matches.subcommand_matches("config") {
        let new_config = Configuration {
            client_id: option_str_string(matches.value_of("client-id")),
            client_secret: option_str_string(matches.value_of("client-secret")),
            input_files: option_str_string(matches.value_of("files"))
        };

        let mut stmt = conn.prepare("SELECT * FROM config").unwrap();
        let mut result = stmt.query(named_params! {}).expect("Failed to query config table");

        let config = match result.next() {
            Ok(Some(row)) => {
                let client_id = row.get::<&str, Option<String>>("client_id").unwrap();
                let client_secret = row.get::<&str, Option<String>>("client_secret").unwrap();
                let input_files = row.get::<&str, Option<String>>("input_files").unwrap();

                let client_id = match new_config.client_id {
                    Some(c) => Some(c),
                    None => client_id
                };

                let client_secret = match new_config.client_secret {
                    Some(c) => Some(c),
                    None => client_secret
                };

                let input_files = match new_config.input_files {
                    Some(c) => Some(c),
                    None => input_files
                };

                Configuration {
                    client_id,
                    client_secret,
                    input_files
                }
            },
            Ok(None) => new_config,
            Err(e) => panic!("{:?}", e)
        };

        match config.is_valid() {
            (false, c) => {
                eprintln!("Configuration is incomplete: {}", c);
                std::process::exit(1);
            },
            _ => {}
        }

        conn.execute("INSERT INTO config (client_id, client_secret, input_files) VALUES (:client_id, :client_secret, :input_files)", named_params! {
            ":client_id": &config.client_id,
            ":client_secret": &config.client_secret,
            ":input_files": &config.input_files
        }).expect("Failed to update table 'config'");

        println!("Configuration updated!");
        std::process::exit(0);
    }

    let config = {
        let mut stmt = conn.prepare("SELECT * FROM config").unwrap();
        let mut result = stmt.query(named_params! {}).expect("Failed to query config table");

        let mut config = None;
        while let Ok(Some(row)) = result.next() {
            let client_id = row.get::<&str, Option<String>>("client_id").unwrap();
            let client_secret = row.get::<&str, Option<String>>("client_secret").unwrap();
            let input_files = row.get::<&str, Option<String>>("input_files").unwrap();

            config = Some(Configuration {
                client_id,
                client_secret,
                input_files
            })
        }

        config.unwrap()
    };

    if let Some(_) = matches.subcommand_matches("show") {
        let mut stmt = conn.prepare("SELECT * FROM config").unwrap();
        let mut result = stmt.query(named_params! {}).expect("Failed to query config table");

        while let Ok(Some(row)) = result.next() {
            let client_id = option_unwrap_text(row.get::<&str, Option<String>>("client_id").unwrap());
            let client_secret = option_unwrap_text(row.get::<&str, Option<String>>("client_secret").unwrap());
            let input_files = option_unwrap_text(row.get::<&str, Option<String>>("input_files").unwrap());

            println!("Current Syncer configuration:");
            println!("Client ID: {}", client_id);
            println!("Client Secret: {}", client_secret);
            println!("Input Files: {}", input_files);
            std::process::exit(0);
        }

        println!("Syncer is unconfigured. Run 'syncer config -h` for more information on how to configure Syncer'");
        std::process::exit(0);
    }

    let _env = Env::new(config.client_id.as_ref().unwrap(), config.client_secret.as_ref().unwrap());


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