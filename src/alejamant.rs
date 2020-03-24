//
// This file is part of elb-dev-tools-ng
//
// Copyright (C) 2020 Eric Le Bihan <eric.le.bihan.dev@free.fr>
//
// SPDX-License-Identifier: MIT OR Apache-2.0
//

use anyhow::{Context, Result};
use serde_derive::Deserialize;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(Debug, Deserialize)]
struct Layout {
    nand: Nand,
    ubi: Ubi,
}

#[derive(Debug, Deserialize)]
struct Nand {
    chip_size: usize,
    block_size: usize,
    partitions: Vec<Partition>,
}

#[derive(Debug, Deserialize)]
struct Ubi {
    beb_limit: u8,
}

#[derive(Debug, Deserialize)]
struct Entry {
    file: PathBuf,
    offset: usize,
}

#[derive(Debug, Deserialize)]
struct Volume {
    name: String,
    file: Option<PathBuf>,
    size: Option<isize>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Partition {
    Raw { name: String, entries: Vec<Entry> },
    Ubi { name: String, volumes: Vec<Volume> },
}

#[derive(Debug, StructOpt)]
#[structopt(name = "alejamant", about = "Compute NAND flash layout")]
struct AlejamantOpts {
    #[structopt(help = "Layout file")]
    input: PathBuf,
}

impl Layout {
    /// Create a new layout from TOML description
    fn from_toml(s: &str) -> Result<Self> {
        let l: Layout = toml::from_str(s)?;
        Ok(l)
    }

    /// Create a new layout from a TOML formatted file
    fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let s = read_to_string(path)?;
        Self::from_toml(&s)
    }
}

fn render(layout: &Layout) -> Result<()> {
    dbg!(&layout);
    Ok(())
}

fn main() -> Result<()> {
    let opts = AlejamantOpts::from_args();
    let layout = Layout::from_path(&opts.input).with_context(|| {
        format!("Failed to create layout from {}", &opts.input.display())
    })?;
    render(&layout).context("Failed to render layout")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_from_toml() {
        let layout = Layout::from_toml(
            r#"
[nand]
chip_size = 536870912
block_size = 131072

[ubi]
beb_limit = 20

[[nand.partitions]]
name = 'ipl'
kind = 'raw'
entries = [
        { file = 'ipl.std.bin', offset = 0x10000 },
        { file = 'ipl.std.bin', offset = 0x20000 },
]

[[nand.partitions]]
name = 'boot'
kind = 'ubi'
volumes = [
        { name = 'spl-std', file = 'spl.std.bin' },
        { name = 'tpl-std', file = 'tpl.std.bin' },
]"#,
        )
        .unwrap();
        assert_eq!(layout.nand.chip_size, 536870912);
        assert_eq!(layout.nand.partitions.len(), 2);
    }
}
