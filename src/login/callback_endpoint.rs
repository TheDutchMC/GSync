//! Actix web endpoint for authorization callback

use actix_web::{get, web, HttpResponse, HttpRequest};
use crate::login::ActixData;
use serde::Deserialize;

///  Struct repres
#[derive(Deserialize)]
struct Query {
    /// Authorization code we can exchange for access tokens
    code:   Option<String>,
    /// Potential errors
    error:  Option<String>,

    /// State parameter which we gave to Google when creating our initial request
    state:  String
}

/// Authorization endpoint
#[get("/")]
pub async fn authorization(data: web::Data<ActixData>, req: HttpRequest) -> HttpResponse {
    let query: Query = match serde_qs::from_str(req.query_string()) {
        Ok(q) => q,
        Err(e) => return HttpResponse::BadRequest().body(e.to_string())
    };

    if let Some(e) = query.error {
        return HttpResponse::BadRequest().body(e);
    }

    let code = match query.code {
        Some(code) => code,
        None => unreachable!()
    };

    if data.state.ne(&query.state) {
        eprintln!("State does noet match!");
        std::process::exit(1);
    }

    match &data.tx.send(code) {
        Ok(_) => HttpResponse::Ok().body("You can now close this tab."),
        Err(e) => {
            eprintln!("Error: Failed to send received code over channel: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}