mod port;
mod callback_endpoint;
pub mod db;

use crate::env::Env;
use actix_web::{HttpServer, App};
use rand::Rng;
use std::sync::mpsc::{Sender, channel};
use crate::api::oauth::LoginData;

use crate::{Result, unwrap_other_err};

#[derive(Clone, Debug)]
pub struct ActixData {
    state:          String,
    tx:             Sender<String>
}

pub fn perform_oauth2_login(env: &Env) -> Result<LoginData> {
    //Generate a code_verifier and code_challenge
    let (code_verifier, code_challenge) = generate_code();
    //Generate a state parameter
    let state = rand::thread_rng().sample_iter(rand::distributions::Alphanumeric).take(32).map(char::from).collect::<String>();

    //Determine a port to listen on
    let port = {
        let mut port = rand::thread_rng().gen_range(4000..8000) as u16;
        while !port::is_free(port) {
            port = rand::thread_rng().gen_range(4000..8000) as u16;
        }

        port
    };

    //This channel will be used to receive the code from the HTTP endpoint
    let (tx_code, rx_code) = channel();
    let actix_data = ActixData { state: state.clone(), tx: tx_code};

    //This channel will be used to receive the Serve instance from Actix
    let (tx_srv, rx_srv) = channel();

    //Start the actix web server and wait for it to return us the Server instance
    std::thread::spawn(move || {
        start_actix(actix_data, port, tx_srv);
    });
    let server = unwrap_other_err!(rx_srv.recv());

    let auth_uri = crate::api::oauth::create_authentication_uri(&env, &code_challenge, &state, &format!("http://localhost:{}", port));

    println!("Info: Please open the following URL:");
    println!("\n{}\n", auth_uri);

    //Wait for the code from the HTTP endpoint
    let code = unwrap_other_err!(rx_code.recv());

    println!("Info: Code received. Exchanging for tokens.");

    //Stop the Actix web server, we dont need it anymore
    actix_web::rt::System::new("").block_on(server.stop(true));

    crate::api::oauth::exchange_access_token(&env, &code, &code_verifier, &format!("http://localhost:{}", port))
}

/// Start the Actix Web Server.
/// This is a blocking method call
/// An instance of Actix's Server will be send over the provided channel so it can be stopped later
fn start_actix(data: ActixData, port: u16, tx: Sender<actix_server::Server>)  {
    let mut sys = actix_web::rt::System::new("Syncer");
    let actix = match HttpServer::new(move || {
        App::new()
            .data(data.clone())
            .service(callback_endpoint::authorization)
    }).bind(format!("0.0.0.0:{}", port)) {
        Ok(s) => s,
        Err(e) => panic!("{:?}", e)
    }.run();

    let _ = tx.send(actix.clone());
    let _ = sys.block_on(actix);
}

/// Generate a code_verifier and code_challenge
fn generate_code() -> (String, String) {
    loop {
        let code_verifier: String = rand::thread_rng().sample_iter(rand::distributions::Alphanumeric).take(96).map(char::from).collect();
        let code_challenge = {
            use sha2::digest::Digest;

            let mut hasher = sha2::Sha256::new();
            hasher.update(code_verifier.as_bytes());
            let digest = hasher.finalize();
            base64::encode(digest.as_slice())
        };

        if code_challenge.contains("+") || code_challenge.contains("/") {
            continue;
        }

        return (code_verifier, code_challenge.replace("=", ""))
    }
}