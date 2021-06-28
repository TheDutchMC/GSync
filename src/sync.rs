use crate::config::Configuration;
use crate::env::Env;
use crate::Result;
use cfg_if::cfg_if;
use std::path::{Path, PathBuf};
use std::fs;
use crate::{unwrap_other_err, unwrap_db_err};
use crate::api::drive;
use rusqlite::named_params;
use std::time::SystemTime;

pub fn sync(config: &Configuration, env: &Env) -> Result<()> {
    // Unwrap is safe because the caller verifiers the configuration
    let input = config.input_files.as_ref().unwrap();
    let input_parts = input.split(",").map(|f| normalize_path(f)).map(|f| PathBuf::from(f)).collect::<Vec<PathBuf>>();

    let mut children = Vec::new();
    for input in input_parts {
        let name = input.clone();
        let name = name.to_str().unwrap();
        println!("Info: Traversing file tree for input '{}'", name);
        let mut ichildren = traverse(input, &mut Vec::new())?;

        let mut child_count = 0i64;
        for child in ichildren.iter() {
            child_count += child.count_all_children();
        }
        println!("Info: Found {} child nodes for input '{}'.", child_count, name);

        children.append(&mut ichildren);
    }

    println!("Info: All directories traversed. Beginning sync now.");

    reset_sync_include(env)?;
    for child in children {
        sync_child(child, env, None)?;
    }

    //remote_delete_removed(env)?;
    Ok(())
}

fn sync_child(child: Child, env: &Env, parent_folder_id: Option<&str>) -> Result<()> {
    match child {
        Child::Directory(dir) => {

            println!("Info: Querying Drive for directory '{}'", &dir.name);
            let query_result = match parent_folder_id {
                Some(parent_folder_id) => drive::list_files(env, Some(&format!("name = '{}' and mimeType = 'application/vnd.google-apps.folder' and trashed = false and '{}' in parents", &dir.name, parent_folder_id)), env.drive_id.as_deref())?,
                None => drive::list_files(env, Some(&format!("name = '{}' and mimeType = 'application/vnd.google-apps.folder' and trashed = false and '{}' in parents", &dir.name, &env.root_folder)), env.drive_id.as_deref())?
            };

            let folder_id = {
                let mut id = String::new();
                for file in query_result {
                    id = file.id;
                }

                if id.is_empty() {
                    println!("Info: Creating directory '{}'", &dir.name);
                    id = match parent_folder_id {
                        Some(parent_folder_id) => drive::create_folder(env, &dir.name, parent_folder_id)?,
                        None => drive::create_folder(env, &dir.name, &env.root_folder)?
                    };
                }

                id
            };

            for child in dir.children {
                sync_child(child, env, Some(&folder_id))?
            }
        },
        Child::File(file_path) => {
            let file_name = file_path.file_name().unwrap().to_str().unwrap();
            println!("Info: Querying Drive for file '{}'", file_name);

            let query_result = match parent_folder_id {
                Some(parent_folder_id) => drive::list_files(env, Some(&format!("name = '{}' and trashed = false and '{}' in parents", file_name, parent_folder_id)), env.drive_id.as_deref())?,
                None => drive::list_files(env, Some(&format!("name = '{}' and trashed = false and '{}' in parents", file_name, &env.root_folder)), env.drive_id.as_deref())?
            };

            match query_result.get(0) {
                Some(file) => {
                    let mod_time_rfc_3339 = &file.modified_time;
                    let mod_time_epoch = unwrap_other_err!(chrono::DateTime::parse_from_rfc3339(mod_time_rfc_3339)).timestamp();

                    if file_changed(&file_path, mod_time_epoch)? {
                        println!("Info: Updating file '{}'", file_name);
                        drive::update_file(env, &file_path, &file.id)?;
                    } else {
                        println!("Info: File '{}' is up-to-date.", file_name);
                    }
                }
                None => {
                    println!("Info: Uploading file '{}'", file_name);
                    match parent_folder_id {
                        Some(parent_folder_id) => drive::upload_file(env, &file_path, &parent_folder_id)?,
                        None => drive::upload_file(env, &file_path, &env.root_folder)?
                    };
                }
            }
        }
    }


    Ok(())
}

fn remote_delete_removed(env: &Env) -> Result<()> {
    let conn = unwrap_db_err!(env.get_conn());
    let mut stmt = unwrap_db_err!(conn.prepare("SELECT path,id FROM files WHERE sync_include = 0"));
    let mut result = unwrap_db_err!(stmt.query(named_params! {}));
    while let Ok(Some(row)) = result.next() {
        let id = unwrap_db_err!(row.get::<&str, String>("id"));
        let path_base64 = unwrap_db_err!(row.get::<&str, String>("path"));
        let path = unwrap_other_err!(String::from_utf8(unwrap_other_err!(base64::decode(path_base64.as_bytes()))));

        println!("Info: Deleting remote file '{}'", path);
        drive::delete_file(env, &id)?;
    }

    unwrap_db_err!(conn.execute("DELETE FROM files WHERE sync_include = `false`", named_params! {}));

    Ok(())
}

fn update_file(path: &Path, env: &Env) -> Result<()> {
    let modification_time = get_modification_time(path)?;
    let path_str = path.to_str().unwrap();
    let base64_path = base64::encode(path_str.as_bytes());

    let conn = unwrap_db_err!(env.get_conn());
    let mut stmt = unwrap_db_err!(conn.prepare("UPDATE files SET modification_time = :mod_time, sync_include = 1 WHERE path = :path"));
    unwrap_db_err!(stmt.execute(named_params! {
        ":mod_time": (modification_time as i64),
        ":path": &base64_path
    }));

    Ok(())
}

fn insert_file(path: &Path, id: &str, env: &Env) -> Result<()> {
    let mod_time = get_modification_time(path)?;
    let path_str = path.to_str().unwrap();
    let path_str = if path_str.ends_with("/") {
        let mut chars = path_str.chars();
        chars.next_back();
        chars.as_str()
    } else {
        path_str
    };

    let base64_path = base64::encode(path_str.as_bytes());

    let conn = unwrap_db_err!(env.get_conn());
    let mut stmt = unwrap_db_err!(conn.prepare("INSERT INTO files (id, path, modification_time, sync_include) VALUES (:id, :path, :mod_time, 1)"));
    unwrap_db_err!(stmt.execute(named_params! {
        ":id": id,
        ":path": base64_path,
        ":mod_time": (mod_time as i64)
    }));

    Ok(())
}

fn get_modification_time(path: &Path) -> Result<u64> {
    let meta = unwrap_other_err!(path.metadata());
    let meta_modified = unwrap_other_err!(meta.modified());
    let as_epoch = unwrap_other_err!(meta_modified.duration_since(SystemTime::UNIX_EPOCH)).as_secs();

    Ok(as_epoch)
}

fn file_changed(path: &Path, stored_modification_time: i64) -> Result<bool> {
    let modification_time = get_modification_time(path)?;
    if modification_time > (stored_modification_time as u64) {
        Ok(true)
    } else {
        Ok(false)
    }
}

fn reset_sync_include(env: &Env) -> Result<()> {
    let conn = unwrap_db_err!(env.get_conn());
    unwrap_db_err!(conn.execute("UPDATE files SET sync_include = 0", named_params! {}));

    Ok(())
}

fn get_file_record(path: &Path, env: &Env) -> Result<Option<(String, i64)>> {
    let conn = unwrap_db_err!(env.get_conn());
    let path_str = path.to_str().unwrap();
    let base64_path = base64::encode(path_str.as_bytes());

    let mut stmt = unwrap_db_err!(conn.prepare("SELECT id,modification_time FROM files WHERE path = :path"));
    let mut result = unwrap_db_err!(stmt.query(named_params! {
        ":path": &base64_path
    }));

    while let Ok(Some(row)) = result.next() {
        let id = unwrap_db_err!(row.get::<&str, String>("id"));
        let modification_time = unwrap_db_err!(row.get::<&str, i64>("modification_time"));

        return Ok(Some((id, modification_time)));
    }

    Ok(None)
}

#[derive(Debug)]
pub struct Directory {
    name:       String,
    path:       PathBuf,
    children:   Vec<Child>
}

#[derive(Debug)]
pub enum Child {
    Directory(Directory),
    File(PathBuf)
}

impl Child {
    fn count_all_children(&self) -> i64 {
        match self {
            Self::File(_) => 1,
            Self::Directory(d) => {
                let mut count = 0i64;
                for child in d.children.iter() {
                    count += child.count_all_children();
                }

                count
            }
        }
    }
}

pub fn traverse(p: PathBuf, exclusions: &mut Vec<PathBuf>) -> Result<Vec<Child>> {
    let mut top_children = Vec::new();

    println!("Info: Traversing '{}'", p.to_str().unwrap());

    if p.is_dir() {
        let mut potential_gitignore = PathBuf::from(&p);
        potential_gitignore.push(".gitignore");
        if potential_gitignore.exists() {
            exclusions.append(&mut parse_gitignore(&potential_gitignore));
        }

        let mut children = Vec::new();
        for entry in unwrap_other_err!(fs::read_dir(&p)) {
            let entry = unwrap_other_err!(entry);

            if exclusions.contains(&entry.path()) { continue }

            let mut ichild = traverse(entry.path(), exclusions)?;
            children.append(&mut ichild);
        }

        top_children.push(Child::Directory(Directory { path: p.clone(), name: p.file_name().unwrap().to_str().unwrap().to_string(), children }))
    } else {
        let file_name = p.file_name().unwrap().to_str().unwrap();
        if file_name.eq(".gitignore") {
            exclusions.append(&mut parse_gitignore(&p))
        }

        top_children.push(Child::File(p));
    }

    Ok(top_children)
}

fn parse_gitignore(p: &Path) -> Vec<PathBuf> {
    let mut exclusions = Vec::new();

    let contents = fs::read_to_string(&p).unwrap();
    for line in contents.lines() {
        if line.is_empty() { continue }
        if line.starts_with("#") { continue }

        let mut line_fmt = line.to_string();
        if line.starts_with("/") { line_fmt = line.replacen("/", "", 1)}
        line_fmt = format!("{}/{}", p.parent().unwrap().to_str().unwrap(), line_fmt);

        exclusions.push(PathBuf::from(line_fmt));
    }

    exclusions
}

fn normalize_path(i: &str) -> String {
    let pwd = pwd();
    if i.starts_with(".") {
        format!("{}{}", pwd, i)
    } else if !i.starts_with("/"){
        format!("{}/{}", pwd, i)
    } else {
        i.to_string()
    }
}

cfg_if! {
    if #[cfg(unix)] {
        fn pwd() -> String {
            let pwd = std::env::var("PWD").unwrap();
            pwd
        }
    } else if #[cfg(windows)] {
        fn pwd() -> String {
            let pwd = std::env::var("cd").unwrap();
            pwd
        }
    } else {
        fn pwd() -> String {
            panic!("Unsupported platform!");
        }
    }
}

#[cfg(test)]
mod test {
    use crate::sync::{pwd, normalize_path};

    #[test]
    fn normalize_path_relative_period() {
        let pwd = pwd();
        let p = "./example";

        assert_eq!(format!("{}{}", pwd, p), normalize_path(p))
    }

    #[test]
    fn normalize_path_relative_no_period() {
        let pwd = pwd();
        let p = "example";

        assert_eq!(format!("{}/{}", pwd, p), normalize_path(p))
    }

    #[test]
    fn normalize_path_absolute() {
        let p = "/tmp/example";

        assert_eq!(p, normalize_path(p))
    }
}