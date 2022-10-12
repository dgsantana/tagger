use chrono::prelude::*;
use colored::*;
use file_patcher::FilePatcher;
use git2::Repository;
use query::Query;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::read_to_string,
    io::Error,
    path::{Path, PathBuf},
    process,
};
use structopt::StructOpt;

mod file_patcher;
mod line_patcher;
mod query;

#[derive(Debug, Default, Serialize, Deserialize)]
struct Patch {
    search: String,
    replace: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PatchFile {
    file: PathBuf,
    change: Vec<Patch>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Config {
    patch: Vec<PatchFile>,
}

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(long = "go")]
    go: bool,
    #[structopt(help = "Path to the config.")]
    config: Option<PathBuf>,
}

fn regex_query_or_die(pattern: &str, replacement: &str, word: bool) -> query::Query {
    let actual_pattern = if word {
        format!(r"\b({})\b", pattern)
    } else {
        pattern.to_string()
    };
    let re = regex::Regex::new(&actual_pattern);
    if let Err(e) = re {
        eprintln!("{}: {}", "Invalid regex".bold().red(), e);
        process::exit(1);
    }
    let re = re.unwrap();
    query::from_regex(re, replacement)
}

fn main() -> Result<(), Error> {
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).ok();
    let opt = Opt::from_args();

    let config_path = opt.config;
    let config_path = config_path.unwrap_or_else(|| Path::new("config.toml").to_path_buf());
    let config = read_to_string(config_path)?;
    let config: Config = toml::from_str(&config)?;
    let go = opt.go;

    let dt = Utc::now().format("%Y/%m/%d %H:%M").to_string();
    let git_rev = match Repository::open(".") {
        Ok(repo) => match repo.revparse_single("HEAD") {
            Ok(reference) => reference.id().to_string(),
            Err(e) => {
                if go {
                    format!("{:?}", e)
                } else {
                    "".to_owned()
                }
            }
        },
        Err(e) => {
            if go {
                format!("{:?}", e)
            } else {
                "".to_owned()
            }
        }
    };
    let git_branch = match Repository::open(".") {
        Ok(repo) => match repo.head() {
            Ok(reference) => reference.name().unwrap_or_default().to_string(),
            Err(e) => {
                if go {
                    format!("{:?}", e)
                } else {
                    "".to_owned()
                }
            }
        },
        Err(e) => {
            if go {
                format!("{:?}", e)
            } else {
                "".to_owned()
            }
        }
    };

    let mut replacement = HashMap::new();
    replacement.insert("@date", dt);
    replacement.insert("@gitrev", git_rev);
    replacement.insert("@gitbranch", git_branch);

    config.patch.iter().for_each(|f| {
        let queries: Vec<Query> = f
            .change
            .iter()
            .map(|p| {
                let mut replace = p.replace.to_string();
                replacement.iter().for_each(|(k, v)| {
                    replace = replace.replace(k, v);
                });
                regex_query_or_die(&p.search, &replace, true)
            })
            .collect();
        let file = f.file.canonicalize().unwrap();
        let file_patcher = FilePatcher::new(file, &queries);
        if let Err(err) = &file_patcher {
            eprintln!("{:?}", err);
        }
        let file_patcher = file_patcher.unwrap();
        let replacements = file_patcher.replacements();
        if replacements.is_empty() {
            println!("It's empty.");
            return;
        }

        file_patcher.print_patch();
        if go {
            file_patcher.run().ok();
        }
    });

    Ok(())
}
