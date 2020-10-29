use crate::tar::tar_gz_entries;
use crate::utils::{content_of, download_to_file};
use indicatif::ProgressIterator;
use log::info;
use overlay::{OverlayDirectory, OverlayFile};
use regex::Regex;
use std::error;
use std::fmt;
use std::io::Read;
use std::path::PathBuf;
use bnf::Grammar;

#[derive(Clone, Debug)]
pub struct OpamError(String);

impl fmt::Display for OpamError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl error::Error for OpamError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

pub struct Opam {
    pub repo: String,
    pub base_path: PathBuf,
    pub archive_url: String,
}

fn parse_index_content(
    index_content: Vec<u8>,
) -> Result<Vec<(String, String, String)>, Box<dyn std::error::Error>> {
    let grammar: Grammar = OPAM_GRAMMAR.parse()?;

    // let mut data = Vec::new();
    let mut result = Vec::new();
    for entry in tar_gz_entries(&index_content).entries()?.take(100) {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();

        if path.to_string_lossy().ends_with("/opam") {
            // data.clear();
            // entry.read_to_end(&mut data)?;
            // let captures = opam_parser
            //     .captures_iter(std::str::from_utf8(&data)?)
            //     .next()
            //     .ok_or(OpamError(format!("failed to decode {:?}, missing src", path)))?;
            // result.push((
            //     path.to_string_lossy().to_string(),
            //     captures[1].to_owned(),
            //     captures[2].to_owned(),
            // ));
        }
    }

    Ok(result)
}

impl Opam {
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let base = OverlayDirectory::new(&self.base_path).await?;

        info!("downloading repo file...");
        let mut repo_file = base.create_file_for_write("repo").await?;
        let repo_content = content_of(format!("{}/repo", self.repo), &mut repo_file).await?;

        info!("downloading repo index...");
        let mut index = base.create_file_for_write("index.tar.gz").await?;
        let index_content = content_of(format!("{}/index.tar.gz", self.repo), &mut index).await?;

        info!("parsing repo index...");
        let all_packages = parse_index_content(index_content)?;

        println!("{:?}", all_packages);
        
        for (name, _, md5) in all_packages {
            let md5_real = &md5[4..];
            let cache_path = format!("md5/{}/{}", &md5_real[..2], md5_real);
            info!("resolving {} from {}", name, cache_path);
            let mut file = base
                .create_file_for_write(format!("archive/{}", cache_path))
                .await?;
            download_to_file(format!("{}/{}", self.archive_url, cache_path), &mut file).await?;
            file.commit().await?;
        }

        index.commit().await?;
        repo_file.commit().await?;
        Ok(())
    }
}
