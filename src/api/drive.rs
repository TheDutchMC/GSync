//! Google Drive API

use serde::{Serialize, Deserialize};
use lazy_static::lazy_static;
use std::sync::{Arc, Mutex};
use std::cell::Cell;
use std::path::Path;
use reqwest::blocking::multipart::{Form, Part};
use crate::api::GoogleResponse;
use crate::api::oauth::get_access_token;

use crate::{Result, unwrap_req_err, unwrap_google_err, unwrap_other_err, Error};
use crate::env::Env;

lazy_static! {
    /// Vector of IDs that can be used for creating files and folders
    static ref IDS: Arc<Mutex<Cell<Vec<String>>>> = Arc::new(Mutex::new(Cell::new(Vec::new())));
}

/// Struct describing the metadata supplied when creating a file
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateFileRequestMetadata<'a> {
    /// The file's name
    name:       &'a str,
    /// The file's MIME type
    mime_type:  &'a str,
    /// The file's ID
    id:         &'a str,
    /// The file's parents
    parents:    Vec<&'a str>
}

/// Create a folder in Google Drive, and return it's ID
///
/// ## Params
/// - `env` Env instance
/// - `folder_name` The name of the folder to create
/// - `parent` ID of parent folder
///
/// ## Errors
/// - Request failure
/// - Google API error
pub fn create_folder(env: &Env, folder_name: &str, parent: &str) -> Result<String> {
    let access_token = get_access_token(env)?;
    let id = get_id(env)?;

    let body = CreateFileRequestMetadata {
        name:       folder_name,
        mime_type:  "application/vnd.google-apps.folder",
        id:         &id,
        parents:    vec![parent]
    };

    let response = unwrap_req_err!(reqwest::blocking::Client::new().post("https://www.googleapis.com/drive/v3/files?supportsAllDrives=true")
        .header("Content-Type","application/json")
        .header("Authorization", &format!("Bearer {}", &access_token))
        .body(serde_json::to_string(&body).unwrap())
        .send());

    let payload: GoogleResponse<()> = unwrap_req_err!(response.json());
    unwrap_google_err!(payload);

    Ok(id)
}

/// Upload a file to Google Drive and return it's ID
///
/// ## Params
/// - `env` Env instance
/// - `path` Path to the file to be uploaded
/// - `parent` ID of the parent folder
///
/// ## Errors
/// - Request failure
/// - Error from Google API
/// - Upon failing to identify MIME type
/// - Upon failing to identify file name
pub fn upload_file<P>(env: &Env, path: P, parent: &str) -> Result<String>
where P: AsRef<Path> {
    let access_token = get_access_token(env)?;
    let id = get_id(env)?;
    let file_name = match path.as_ref().file_name() {
        Some(f) => f.to_str().unwrap(),
        None => return Err((Error::Other("Missing file name".to_string()), line!(), file!()))
    };

    let mime = match mime_guess::from_path(&path).first() {
        Some(g) => {
            g.essence_str().to_string()
        },
        None => "application/octet-stream".to_string()
    };

    let body = CreateFileRequestMetadata {
        name:       file_name,
        parents:    vec![parent],
        id:         &id,
        mime_type:  &mime
    };

    let metadata_part = unwrap_req_err!(Part::text(serde_json::to_string(&body).unwrap()).mime_str("application/json"));
    let file_part = unwrap_req_err!(unwrap_other_err!(Part::file(path)).mime_str(&mime));

    let form = Form::new()
        .part("Metadata", metadata_part)
        .part("Media", file_part);

    let response = unwrap_req_err!(reqwest::blocking::Client::new().post("https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart&supportsAllDrives=true")
        .multipart(form)
        .header("Content-Type", "multipart/related")
        .header("Authorization", &format!("Bearer {}", &access_token))
        .send());

    let payload: GoogleResponse<()> = unwrap_req_err!(response.json());
    unwrap_google_err!(payload);

    Ok(id)
}

/// Struct describing the request the the file list API
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileListRequest<'a> {
    /// Search query parameter
    #[serde(skip_serializing_if = "Option::is_none")]
    q:                              Option<&'a str>,

    /// The ID of the drive to search in
    #[serde(skip_serializing_if = "Option::is_none")]
    drive_id:                       Option<&'a str>,

    /// The Corpora
    corpora:                        &'static str,

    /// If we support all drives, we do
    supports_all_drives:            bool,

    /// Do we include items from all drives, no, we don't
    include_items_from_all_drives:  bool,

    /// The fields to get
    fields:                         &'static str
}

/// Struct describing the response to a call to the list API
#[derive(Deserialize, Debug)]
struct FileListResponse {
    /// The files returned
    files:  Vec<File>
}

/// Struct describing an individual file returned by the list API
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct File {
    /// The ID of the file
    pub id:             String,
    /// The name of the file
    pub name:           String,
    /// The time the file was last modified
    pub modified_time:  String,
}

/// List the files in Google Drive
///
/// ## Params
/// - `env` Env instance
/// - `q` Search parameter, refer to [Google docs](https://developers.google.com/drive/api/v3/search-files)
/// - `drive_id` If Team Drive, the ID of that Team Drive
///
/// ## Error
/// - Request failure
/// - Error from Google API
pub fn list_files(env: &Env, q: Option<&str>, drive_id: Option<&str>) -> Result<Vec<File>> {
    let query_params = FileListRequest {
        q,
        drive_id,
        corpora:                        if drive_id.is_some() { "drive" } else { "user" },
        supports_all_drives:            true,
        include_items_from_all_drives:  true,
        fields:                         "kind,incompleteSearch,files/kind,files/modifiedTime,files/id,files/name"
    };

    let access_token = get_access_token(env)?;
    let req = unwrap_req_err!(reqwest::blocking::Client::new().get(format!("https://www.googleapis.com/drive/v3/files?{}", serde_qs::to_string(&query_params).unwrap()))
        .header("Authorization", &format!("Bearer {}", &access_token))
        .send());

    let request_payload: GoogleResponse<FileListResponse> = unwrap_req_err!(req.json());
    let payload = unwrap_google_err!(request_payload);

    Ok(payload.files)
}

/// Struct describing the response to the shared drives API
#[derive(Deserialize, Debug)]
struct SharedDriveResponse {
    /// The returned drives
    drives: Vec<SharedDrive>,
}

/// Struct describing the individual drives returned by the shared shared drives API
#[derive(Deserialize, Debug)]
pub struct SharedDrive {
    /// The drive's ID
    pub id:     String,
    /// The drive's name
    pub name:   String
}

/// Get all shared drives the user has access too
///
/// # Error
/// - Google API error
/// - Reqwest error
pub fn get_shared_drives(env: &Env) -> Result<Vec<SharedDrive>> {
    let access_token = get_access_token(env)?;

    let request = unwrap_req_err!(reqwest::blocking::Client::new().get("https://www.googleapis.com/drive/v3/drives?pageSize=100")
        .header("Authorization", &format!("Bearer {}", &access_token))
        .send());

    let response: GoogleResponse<SharedDriveResponse> = unwrap_req_err!(request.json());
    let payload = unwrap_google_err!(response);

    Ok(payload.drives)
}

/// Struct describing the response to a call to the generateIds API
#[derive(Deserialize)]
struct GetIdsResponse {
    /// The returned IDs
    ids:    Vec<String>
}

/// Get a File ID from the IDS Vec. If this Vec contains no more IDs, a new set will be requested from Google.
///
/// ## Params
/// - `env` Env instance
///
/// ## Errors
/// - Request failure
/// - Error from Google API
fn get_id(env: &Env) -> Result<String> {
    let mut lock = unwrap_other_err!(IDS.lock());
    let vec = lock.get_mut();

    let access_token = get_access_token(env)?;

    if vec.is_empty() {
        let mut new_ids = get_ids_from_google(&access_token)?;
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

/// Struct describing the query parameters used when updating a file
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateFileRequestQuery {
    /// The upload type
    upload_type:            &'static str,
    /// If we support all drives, we do
    supports_all_drives:    bool
}

/// Struct describing the metadata used when updating a file
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateFileRequest<'a> {
    /// The MIME type of the file
    mime_type: &'a str
}

/// Update a file in Google Drive. The caller should make sure the file exists.
///
/// ## Params
/// - `env` Env instance
/// - `path` Path to the file to be updated
/// - `id` The ID of the existing file in Google Drive to be updated
///
/// ## Errors
/// - Request failure
/// - Google API error
/// - Failure to construct multipart parts
pub fn update_file<P>(env: &Env, path: P, id: &str) -> Result<()>
where P: AsRef<Path> {
    let access_token = get_access_token(env)?;
    let query = UpdateFileRequestQuery {
        supports_all_drives:    true,
        upload_type:            "multipart"
    };

    let mime = match mime_guess::from_path(&path).first() {
        Some(g) => {
            g.essence_str().to_string()
        },
        None => "application/octet-stream".to_string()
    };

    let payload = UpdateFileRequest {
        mime_type: &mime
    };

    let metadata_part = unwrap_req_err!(Part::text(unwrap_other_err!(serde_json::to_string(&payload))).mime_str("application/json"));
    let file_part = unwrap_req_err!(unwrap_other_err!(Part::file(&path)).mime_str(&mime));

    let form = Form::new()
        .part("Metadata", metadata_part)
        .part("Media", file_part);

    let uri = format!("https://www.googleapis.com/upload/drive/v3/files/{}?{}", id, unwrap_other_err!(serde_qs::to_string(&query)));
    let response = unwrap_req_err!(reqwest::blocking::Client::new().patch(&uri)
        .multipart(form)
        .header("Content-Type", "multipart/related")
        .header("Authorization", &format!("Bearer {}", access_token))
        .send());

    let payload: GoogleResponse<()> = unwrap_req_err!(response.json());
    unwrap_google_err!(payload);

    Ok(())
}

/// Permanently delete a file
///
/// ## Params
/// - `env` Env instance
/// - `id` The ID of the existing file in Google Drive to be updated
///
/// ## Errors
/// - Request failure
/// - Google API error
pub fn delete_file(env: &Env, id: &str) -> Result<()> {
    let access_token = get_access_token(env)?;
    let uri = format!("https://www.googleapis.com/drive/v3/files/{}?supportsAllDrives=true", id);
    let response = unwrap_req_err!(reqwest::blocking::Client::new().delete(&uri)
        .header("Authorization", &format!("Bearer {}", access_token))
        .send());

    let payload: GoogleResponse<()> = unwrap_req_err!(response.json());
    unwrap_google_err!(payload);

    Ok(())
}