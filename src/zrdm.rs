//
// This file is part of elb-dev-tools-ng
//
// Copyright (C) 2020 Eric Le Bihan <eric.le.bihan.dev@free.fr>
//
// SPDX-License-Identifier: MIT OR Apache-2.0
//

use failure::{self, format_err};
use flate2::read::GzDecoder;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use tar::Archive;
use tempfile::tempdir;

fn is_readme_filename(path: &Path) -> bool {
    path.to_str().map_or(false, |s| {
        let filenames = &["readme", "readme.md", "readme.txt"];
        filenames.iter().any(|f| f.eq_ignore_ascii_case(s))
    })
}

#[derive(Debug, StructOpt)]
#[structopt(name = "zrdm", about = "Display README from tarball")]
struct ZrdmOpts {
    #[structopt(help = "Archive to explore")]
    tarball: PathBuf,
}

fn main() -> Result<(), failure::Error> {
    let opts = ZrdmOpts::from_args();
    let file = File::open(&opts.tarball)?;
    let mut archive = Archive::new(GzDecoder::new(file));
    let mut candidates = archive
        .entries()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            if let Ok(path) = entry.path() {
                let path: PathBuf = path.components().skip(1).collect();
                return is_readme_filename(&path);
            }
            false
        });

    let tmpdir = tempdir()?;

    let path = candidates
        .next()
        .ok_or(format_err!("No README found"))
        .and_then(|mut entry| {
            if let Ok(path) = entry.path() {
                let path: PathBuf = path.components().skip(1).collect();
                let path = tmpdir.path().join(path);
                entry
                    .unpack(&path)
                    .map_err(|e| format_err!("Failed to unpack {}", e))?;
                Ok(path)
            } else {
                Err(format_err!("Invalid path"))
            }
        })?;

    File::open(&path)
        .map_err(|e| format_err!("Failed to open ({})", e))
        .and_then(|mut f| {
            let stdout = io::stdout();
            let mut stdout = stdout.lock();
            io::copy(&mut f, &mut stdout)
                .map_err(|e| format_err!("Failed to output ({})", e))
        })
        .and_then(|_| {
            fs::remove_file(&path)
                .map_err(|e| format_err!("Failed to remove file ({})", e))
        })
}
