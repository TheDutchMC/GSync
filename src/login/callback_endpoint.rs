use actix_web::{get, web, HttpResponse, HttpRequest};
use crate::login::ActixData;
use serde::Deserialize;

#[derive(Deserialize)]
struct Query {
    code:   Option<String>,
    error:  Option<String>,
    state:  String
}

#[get("/")]
pub async fn authorization(data: web::Data<ActixData>, req: HttpRequest) -> HttpResponse {
    let query: Query = match serde_qs::from_str(req.query_string()) {
        Ok(q) => q,
        Err(e) => return HttpResponse::BadRequest().body(e.to_string())
    };

    match query.error {
        Some(e) => return HttpResponse::BadRequest().body(e),
        None => {}
    };

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