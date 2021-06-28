//! Module related to syncing files

use crate::config::Configuration;
use crate::env::Env;
use crate::Result;
use cfg_if::cfg_if;
use std::path::{Path, PathBuf};
use std::fs;
use crate::unwrap_other_err;
use crate::api::drive;
use std::time::SystemTime;

/// Sync the configured input files to google drive
pub fn sync(config: &Configuration, env: &Env) -> Result<()> {
    // Unwrap is safe because the caller verifiers the configuration
    let input = config.input_files.as_ref().unwrap();
    let input_parts = input.split(',').map(|f| normalize_path(f)).map(PathBuf::from).collect::<Vec<PathBuf>>();

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

    for child in children {
        sync_child(child, env, None)?;
    }

    Ok(())
}

/// Delete a file from Google Drive if it no longer exists locally
fn delete_if_removed(path: &Path, parent_id: &str, env: &Env) -> Result<()> {
    if !path.exists() {
        let name = path.file_name().unwrap().to_str().unwrap();
        let file_list  = drive::list_files(env, Some(&format!("name = '{}' and '{}' in parents", name, parent_id)), env.drive_id.as_deref())?;
        for file in file_list {
            drive::delete_file(env, &file.id)?;
        }
    }

    Ok(())
}

/// Sync a child with Google Drive. This is a recursive function
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
                        Some(pfi) => drive::create_folder(env, &dir.name, pfi)?,
                        None => drive::create_folder(env, &dir.name, &env.root_folder)?
                    }
                }

                id
            };

            match parent_folder_id {
                Some(pfi) => delete_if_removed(&dir.path, pfi, env)?,
                None => delete_if_removed(&dir.path, &env.root_folder, env)?
            }

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
                        Some(pfi) => drive::upload_file(env, &file_path, pfi)?,
                        None => drive::upload_file(env, &file_path, &env.root_folder)?
                    };
                }
            }
        }
    }

    Ok(())
}

/// Get the modification time of a file
///
/// # Errors
/// - When the underlying IO operation to fetch the modification time fails
fn get_modification_time(path: &Path) -> Result<u64> {
    let meta = unwrap_other_err!(path.metadata());
    let meta_modified = unwrap_other_err!(meta.modified());
    let as_epoch = unwrap_other_err!(meta_modified.duration_since(SystemTime::UNIX_EPOCH)).as_secs();

    Ok(as_epoch)
}

/// Check if a file has changed by their modification time
///
/// # Errors
/// - When the underlying IO operation to fetch the modification time fails
fn file_changed(path: &Path, stored_modification_time: i64) -> Result<bool> {
    let modification_time = get_modification_time(path)?;
    if modification_time > (stored_modification_time as u64) {
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Struct describing a Directory
#[derive(Debug)]
pub struct Directory {
    /// The name of the directory
    name:       String,

    /// The path to the directory
    path:       PathBuf,

    /// A vector of Child's that this directory is the parent of
    children:   Vec<Child>
}

/// Enum describing a Child
#[derive(Debug)]
pub enum Child {
    /// Directory
    Directory(Directory),

    /// File
    File(PathBuf)
}

impl Child {
    /// Cound all Child elements to this Child
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

/// Traverse a path to map them to a Vec of Child
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

/// Parse a gitignore file, returns a Vec<PathBuf> to be ignored
fn parse_gitignore(p: &Path) -> Vec<PathBuf> {
    let mut exclusions = Vec::new();

    let contents = fs::read_to_string(&p).unwrap();
    for line in contents.lines() {
        if line.is_empty() { continue }
        if line.starts_with('#') { continue }

        let mut line_fmt = line.to_string();
        if line.starts_with('/') { line_fmt = line.replacen("/", "", 1)}
        line_fmt = format!("{}/{}", p.parent().unwrap().to_str().unwrap(), line_fmt);

        exclusions.push(PathBuf::from(line_fmt));
    }

    exclusions
}

/// Normalize a path. Meaning a relative path will be turned into an absolute one.
fn normalize_path(i: &str) -> String {
    // Clippy is a bit odd here, so we'll just allow it
    #![allow(clippy::if_not_else)]

    let pwd = pwd();
    if i.starts_with('.') {
        format!("{}{}", pwd, i)
    } else if !i.starts_with('/') {
        format!("{}/{}", pwd, i)
    } else {
        i.to_string()
    }
}

cfg_if! {
    if #[cfg(unix)] {
        /// Get the current working directory
        fn pwd() -> String {
            std::env::var("PWD").unwrap()
        }
    } else if #[cfg(windows)] {
        /// Get the current working directory
        fn pwd() -> String {
            std::env::var("cd").unwrap()
        }
    } else {
        /// Get the current working directory
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