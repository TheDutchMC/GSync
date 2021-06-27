use crate::config::Configuration;
use crate::env::Env;
use crate::Result;
use cfg_if::cfg_if;
use std::path::{Path, PathBuf};
use std::fs;
use crate::{unwrap_other_err};
use crate::api::drive;

pub mod files;

pub fn sync(config: &Configuration, env: &Env) -> Result<()> {
    // Unwrap is safe because the caller verifiers the configuration
    let input = config.input_files.as_ref().unwrap();
    let input_parts = input.split(",").map(|f| normalize_path(f)).map(|f| PathBuf::from(f)).collect::<Vec<PathBuf>>();

    let mut children = Vec::new();
    for input in input_parts {
        let mut ichildren = traverse(input, &mut Vec::new())?;
        children.append(&mut ichildren);
    }

    let access_token = crate::api::oauth::get_access_token(env)?;


    Ok(())
}

#[derive(Debug)]
pub struct Directory {
    name:       String,
    children:   Vec<Child>
}

#[derive(Debug)]
pub enum Child {
    Directory(Directory),
    File(PathBuf)
}

pub fn traverse(p: PathBuf, exclusions: &mut Vec<PathBuf>) -> Result<Vec<Child>> {
    let mut top_children = Vec::new();

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

        top_children.push(Child::Directory(Directory { name: p.file_name().unwrap().to_str().unwrap().to_string(), children }))
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
        line_fmt = format!("{}/{}", p.parent().unwrap().to_str().unwrap(), line);

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