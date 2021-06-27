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

/// Create a folder in Google Drive, and return it's ID
///
/// ## Params
/// - `access_token` OAuth2 access token
/// - `folder_name` The name of the folder to create
/// - `parent` ID of parent folder
///
/// ## Errors
/// - Request failure
/// - Google API error
pub fn create_folder(access_token: &str, folder_name: &str, parent: &str) -> Result<String> {
    let id = get_id(access_token)?;

    let body = CreateFileRequestMetadata {
        name:       folder_name,
        mime_type:  "application/vnd.google-apps.folder",
        id:         &id,
        parents:    vec![parent]
    };

    let response = unwrap_req_err!(reqwest::blocking::Client::new().post("https://www.googleapis.com/drive/v3/files")
        .header("Content-Type","application/json")
        .header("Authorization", &format!("Bearer {}", access_token))
        .body(serde_json::to_string(&body).unwrap())
        .send());

    let payload: GoogleResponse<()> = unwrap_req_err!(response.json());
    unwrap_google_err!(payload);

    Ok(id)
}

/// Upload a file to Google Drive and return it's ID
///
/// ## Params
/// - `access_token` OAuth2 access token
/// - `path` Path to the file to be uploaded
/// - `parent` ID of the parent folder
///
/// ## Errors
/// - Request failure
/// - Error from Google API
/// - Upon failing to identify MIME type
/// - Upon failing to identify file name
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

    let metadata_part = unwrap_req_err!(Part::text(serde_json::to_string(&body).unwrap()).mime_str("application/json"));
    let file_part = unwrap_req_err!(unwrap_other_err!(Part::file(path)).mime_str(mime));

    let form = Form::new()
        .part("Metadata", metadata_part)
        .part("Media", file_part);

    let response = unwrap_req_err!(reqwest::blocking::Client::new().post("https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart")
        .multipart(form)
        .header("Content-Type", "multipart/related")
        .header("Authorization", &format!("Bearer {}", access_token))
        .send());

    let payload: GoogleResponse<()> = unwrap_req_err!(response.json());
    unwrap_google_err!(payload);

    Ok(id)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileListRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    q:                              Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    drive_id:                       Option<&'a str>,

    corpora:                        &'static str,

    supports_all_drives:            bool,
    include_items_from_all_drives:  bool
}

#[derive(Deserialize, Debug)]
struct FileListResponse {
    files:  Vec<File>
}

#[derive(Deserialize, Debug)]
pub struct File {
    pub id:     String,
    pub name:   String,
}

/// List the files in Google Drive
///
/// ## Params
/// - `access_token` OAuth2 access token
/// - `q` Search parameter, refer to [Google docs](https://developers.google.com/drive/api/v3/search-files)
/// - `drive_id` If Team Drive, the ID of that Team Drive
///
/// ## Error
/// - Request failure
/// - Error from Google API
pub fn list_files(access_token: &str, q: Option<&str>, drive_id: Option<&str>) -> Result<Vec<File>> {
    let query_params = FileListRequest {
        q,
        drive_id,
        corpora:                        if drive_id.is_some() { "drive" } else { "user" },
        supports_all_drives:            true,
        include_items_from_all_drives:  true
    };

    let req = unwrap_req_err!(reqwest::blocking::Client::new().get(format!("https://www.googleapis.com/drive/v3/files?{}", serde_qs::to_string(&query_params).unwrap()))
        .header("Authorization", &format!("Bearer {}", access_token))
        .send());

    let request_payload: GoogleResponse<FileListResponse> = unwrap_req_err!(req.json());
    let payload = unwrap_google_err!(request_payload);

    Ok(payload.files)
}

#[derive(Deserialize)]
struct GetIdsResponse {
    ids:    Vec<String>
}

/// Get a File ID from the IDS Vec. If this Vec contains no more IDs, a new set will be requested from Google.
///
/// ## Params
/// - `access_token` OAuth2 access token
///
/// ## Errors
/// - Request failure
/// - Error from Google API
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

/// Request 100 new File IDs from Google. Do not call this function directly, instead use `get_id()`
///
/// ## Errors
/// - Request failure
/// - Error from Google API
fn get_ids_from_google(access_token: &str) -> Result<Vec<String>> {
    let request = unwrap_req_err!(reqwest::blocking::Client::new().get("https://www.googleapis.com/drive/v3/files/generateIds?count=100")
        .header("Authorization", &format!("Bearer {}", access_token))
        .send());

    let payload: GoogleResponse<GetIdsResponse> = unwrap_req_err!(request.json());
    let ids = unwrap_google_err!(payload);
    Ok(ids.ids)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateFileRequestQuery {
    upload_type:            &'static str,
    supports_all_drives:    bool
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateFileRequest<'a> {
    mime_type: &'a str
}

/// Update a file in Google Drive. The caller should make sure the file exists.
///
/// ## Params
/// - `access_token` OAuth2 access token
/// - `path` Path to the file to be updated
/// - `id` The ID of the existing file in Google Drive to be updated
///
/// ## Errors
/// - Request failure
/// - Google API error
/// - Failure to construct multipart parts
pub fn update_file<P>(access_token: &str, path: P, id: &str) -> Result<()>
where P: AsRef<Path> {

    let query = UpdateFileRequestQuery {
        supports_all_drives:    true,
        upload_type:            "multipart"
    };

    let mime_guess = mime_guess::from_path(&path).first().unwrap();
    let mime = mime_guess.essence_str();

    let payload = UpdateFileRequest {
        mime_type: mime
    };

    let metadata_part = unwrap_req_err!(Part::text(unwrap_other_err!(serde_json::to_string(&payload))).mime_str("application/json"));
    let file_part = unwrap_req_err!(unwrap_other_err!(Part::file(&path)).mime_str(mime));

    let form = Form::new()
        .part("Metadata", metadata_part)
        .part("Media", file_part);

    let uri = format!("https://www.googleapis.com/upload/drive/v3/files/{}?{}", id, unwrap_other_err!(serde_qs::to_string(&query)));
    let response = unwrap_req_err!(reqwest::blocking::Client::new().patch(&uri)
        .multipart(form)
        .header("Content-Type", "multipart/related")
        .header("Authorization", &format!("Bearerr {}", access_token))
        .send());

    let payload: GoogleResponse<()> = unwrap_req_err!(response.json());
    unwrap_google_err!(payload);

    Ok(())
}

/// Permanently delete a file
///
/// ## Params
/// - `access_token` OAuth2 access token
/// - `id` The ID of the existing file in Google Drive to be updated
///
/// ## Errors
/// - Request failure
/// - Google API error
pub fn delete_file(access_token: &str, id: &str) -> Result<()> {
    let uri = format!("https://www.googleapis.com/drive/v3/files/{}?supportsAllDrives=true", id);
    let response = unwrap_req_err!(reqwest::blocking::Client::new().delete(&uri)
        .header("Authorization", &format!("Bearerr {}", access_token))
        .send());

    let payload: GoogleResponse<()> = unwrap_req_err!(response.json());
    unwrap_google_err!(payload);

    Ok(())
}