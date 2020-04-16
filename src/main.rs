use chrono::prelude::*;
use colored::*;
use file_patcher::FilePatcher;
use git2::Repository;
use serde::{Deserialize, Serialize};
use std::{
    fs::read_to_string,
    io::{Error, ErrorKind},
    path::{Path, PathBuf},
    process,
};
use structopt::StructOpt;
use toml;

mod file_patcher;
mod line_patcher;
mod query;

#[derive(Debug, Default, Serialize, Deserialize)]
struct PatchFile {
    file: PathBuf,
    search: String,
    replace: String,
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
    let opt = Opt::from_args();

    let config_path = opt.config;
    let config_path = config_path.unwrap_or_else(|| Path::new("config.toml").to_path_buf());
    let config = read_to_string(config_path)?;
    let config: Config = toml::from_str(&config)?;

    let dt = Utc::now().format("%Y/%m/%d %H:%M").to_string();
    let gitrev = match Repository::init(".") {
        Ok(repo) => match repo.revparse_single("HEAD") {
            Ok(reference) => reference.id().to_string(),
            Err(e) => format!("{:?}", e).to_owned(),
        },
        Err(e) => format!("{:?}", e).to_owned(),
    };

    let go = opt.go;
    config.patch.iter().for_each(|f| {
        let replace = &f.replace;
        let replace = replace.replace("@date", &dt);
        let replace = replace.replace("@gitrev", &gitrev);
        let query = regex_query_or_die(&f.search, &replace, true);
        let file_patcher = FilePatcher::new(f.file.to_path_buf(), &query);
        if let Err(err) = &file_patcher {
            println!("{:?}", err);
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
