//
// This file is part of elb-dev-tools-ng
//
// Copyright (C) 2020 Eric Le Bihan <eric.le.bihan.dev@free.fr>
//
// SPDX-License-Identifier: MIT OR Apache-2.0
//

use anyhow::{anyhow, Result};
use std::process::Command;
use std::str;

pub fn run_command_or(command: &mut Command, error: &str) -> Result<String> {
    let output = command.output()?;

    if !output.status.success() {
        return Err(anyhow!(error.to_string()));
    }

    let text = str::from_utf8(&output.stdout)?.trim_end().to_string();
    Ok(text)
}
