use serde::{Serialize, Deserialize};
use lazy_static::lazy_static;
use std::sync::{Arc, Mutex};
use std::cell::Cell;
use std::path::Path;
use reqwest::blocking::multipart::{Form, Part};
use crate::api::GoogleResponse;

use crate::{Result, unwrap_req_err, unwrap_google_err, unwrap_other_err};

lazy_static! {
    static ref IDS: Arc<Mutex<Cell<Vec<String>>>> = Arc::new(Mutex::new(Cell::new(Vec::new())));
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateFileRequestMetadata<'a> {
    name:       &'a str,
    mime_type:  &'a str,
    id:         &'a str,
    parents:    Vec<&'a str>
}

pub fn create_folder(access_token: &str, folder_name: &str, parent: &str) -> Result<String> {
    let id = get_id(access_token)?;

    let body = CreateFileRequestMetadata {
        name:       folder_name,
        mime_type:  "application/vnd.google-apps.folder",
        id:         &id,
        parents:    vec![parent]
    };

    unwrap_req_err!(reqwest::blocking::Client::new().post("https://www.googleapis.com/drive/v3/files")
        .header("Content-Type","application/json")
        .header("Authorization", &format!("Bearer {}", access_token))
        .body(serde_json::to_string(&body).unwrap())
        .send());

    Ok(id)
}

pub fn upload_file<P>(access_token: &str, path: P, parent: &str) -> Result<String>
where P: AsRef<Path> {

    let id = get_id(access_token)?;
    let file_name = match path.as_ref().file_name() {
        Some(f) => f.clone(),
        None => panic!("TODO: FILE NAME NONE")
    }.to_str().unwrap();

    let mime_guess = mime_guess::from_path(&path).first().unwrap();
    let mime = mime_guess.essence_str();

    let body = CreateFileRequestMetadata {
        name:       file_name,
        parents:    vec![parent],
        id:         &id,
        mime_type:  mime
    };

    let metadata_part = unwrap_other_err!(Part::text(serde_json::to_string(&body).unwrap()).mime_str("application/json"));
    let file_part = unwrap_other_err!(unwrap_other_err!(Part::file(path)).mime_str(mime));

    let form = Form::new()
        .part("Metadata", metadata_part)
        .part("Media", file_part);

    unwrap_req_err!(reqwest::blocking::Client::new().post("https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart")
        .multipart(form)
        .header("Content-Type", "multipart/related")
        .header("Authorization", &format!("Bearer {}", access_token))
        .send());

    Ok(id)
}

#[derive(Deserialize)]
struct GetIdsResponse {
    ids:    Vec<String>
}

fn get_id(access_token: &str) -> Result<String> {
    let mut lock = unwrap_other_err!(IDS.lock());
    let vec = lock.get_mut();
    if vec.len() == 0 {
        let mut new_ids = get_ids_from_google(access_token)?;
        let id = new_ids.pop().unwrap();
        lock.set(new_ids);

        return Ok(id);
    }

    Ok(vec.pop().unwrap())
}

fn get_ids_from_google(access_token: &str) -> Result<Vec<String>> {
    let request = unwrap_req_err!(reqwest::blocking::Client::new().get("https://www.googleapis.com/drive/v3/files/generateIds?count=100")
        .header("Authorization", &format!("Bearer {}", access_token))
        .send());

    let payload: GoogleResponse<GetIdsResponse> = unwrap_req_err!(request.json());
    let ids = unwrap_google_err!(payload);
    Ok(ids.ids)
}