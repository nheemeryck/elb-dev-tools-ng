//
// This file is part of elb-dev-tools-ng
//
// Copyright (C) 2020 Eric Le Bihan <eric.le.bihan.dev@free.fr>
//
// SPDX-License-Identifier: MIT OR Apache-2.0
//

use dirs;
use failure::format_err;
use handlebars::{no_escape, Handlebars};
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;
use structopt::StructOpt;

const DEFAULT_TEMPLATE: &str = r"From: {{emitter}}
To: {{recipients}}
Subject: [{{prefix}}] {{project}} {{version}} is available
bcc: {{emitter}}

Hi!

Version {{version}} of {{project}} is available in its repository [1].

[1] {{url}}

{{text}}

Regards,

{{signature}}
";

#[derive(Debug, StructOpt)]
#[structopt(name = "kemenn", about = "Announce a project release")]
struct KemennOpts {
    #[structopt(
        short = "f",
        long = "from",
        help = "Emitter email address",
        value_name = "EMAIL"
    )]
    emitter: Option<String>,

    #[structopt(
        short = "c",
        long = "changelog",
        help = "Name of changelog",
        value_name = "NAME",
        parse(from_os_str)
    )]
    changelog: Option<PathBuf>,

    #[structopt(
        short = "t",
        long = "template",
        help = "Path to mail template",
        value_name = "PATH"
    )]
    template: Option<PathBuf>,

    #[structopt(
        short = "i",
        long = "input",
        help = "Path to recipients file",
        value_name = "PATH",
        parse(from_os_str)
    )]
    input: Option<PathBuf>,

    #[structopt(
        short = "o",
        long = "output",
        help = "Path to output file",
        value_name = "PATH",
        parse(from_os_str)
    )]
    output: Option<PathBuf>,

    #[structopt(
        short = "P",
        long = "parameter",
        help = "Extra K:V key value pair",
        number_of_values = 1,
        value_name = "STRING"
    )]
    parameters: Option<Vec<String>>,

    #[structopt(help = "Repository")]
    repository: PathBuf,

    #[structopt(help = "Recipients")]
    recipients: Vec<String>,
}

/// Represent information about a release
#[derive(Debug)]
struct ReleaseInfo {
    project: String,
    url: String,
    version: String,
    changelog: String,
}

fn run_command_or(
    command: &mut Command,
    error: Box<dyn Error>,
) -> Result<String, Box<dyn Error>> {
    let output = command.output()?;

    if !output.status.success() {
        return Err(error);
    }

    let text = str::from_utf8(&output.stdout)?.trim_end().to_string();
    Ok(text)
}

fn get_repo_url<P: AsRef<Path>>(path: P) -> Result<String, Box<dyn Error>> {
    let mut cmd = Command::new("git");
    cmd.arg("--git-dir")
        .arg(path.as_ref())
        .arg("config")
        .arg("--get")
        .arg("remote.origin.url");

    run_command_or(&mut cmd, format_err!("git config failed").into())
}

fn get_repo_version<P: AsRef<Path>>(path: P) -> Result<String, Box<dyn Error>> {
    let mut cmd = Command::new("git");
    cmd.arg("--git-dir")
        .arg(path.as_ref())
        .arg("describe")
        .arg("--abbrev=0")
        .arg("--tags");

    run_command_or(&mut cmd, format_err!("git describe failed").into())
}

fn get_repo_changelog<P: AsRef<Path>>(
    path: P,
    version: &str,
) -> Result<String, Box<dyn Error>> {
    let pattern =
        format!(r"^##\s+\[{}]\s+-\s+[\d]{{4}}-[\d]{{2}}-[\d]{{2}}$", version);
    let pattern = Regex::new(pattern.as_str())?;
    let input = File::open(&path)?;
    let reader = BufReader::new(input);
    let mut found = false;
    let mut text = String::new();
    for line in reader.lines() {
        let line = line?;
        if !found {
            found = pattern.is_match(&line);
        } else {
            if line.starts_with("## ") {
                break;
            }
            text.push_str(&line);
            text.push('\n');
        }
    }
    Ok(text)
}

fn get_project_name(url: &str) -> Option<String> {
    let project = url.split('/').last()?;
    let name = match project.find(".git") {
        Some(pos) => String::from(&project[..pos]),
        None => project.to_string(),
    };
    Some(name)
}

/// Represent a project
#[derive(Debug)]
struct Project {
    path: PathBuf,
    changelog: PathBuf,
}

impl Project {
    /// Create a `Project`
    fn new<P: AsRef<Path>>(path: P) -> Self {
        Project {
            path: PathBuf::from(path.as_ref()),
            changelog: PathBuf::from("NEWS.md"),
        }
    }

    /// Explore to get latest release information
    fn latest_release(&self) -> Result<ReleaseInfo, Box<dyn Error>> {
        let mut gitdir = PathBuf::from(&self.path);
        gitdir.push(".git");
        let url = get_repo_url(&gitdir)?;
        let version = get_repo_version(&gitdir)?;
        let project = get_project_name(&url)
            .ok_or(format_err!("Failed to extract project name from URL"))?;
        let mut path = PathBuf::from(&self.path);
        path.push(&self.changelog);
        let changelog = get_repo_changelog(&path, &version)?;
        let info = ReleaseInfo {
            project: project,
            url: url,
            version: version,
            changelog: changelog,
        };
        Ok(info)
    }

    fn set_changelog<P: AsRef<Path>>(&mut self, filename: P) {
        self.changelog = PathBuf::from(filename.as_ref());
    }
}

/// Collect data to fill mail template
#[derive(Debug)]
struct MailDataBuilder {
    data: HashMap<String, String>,
}

impl MailDataBuilder {
    /// Create a `MailDataBuilder`
    fn new() -> Self {
        let mut data = HashMap::new();
        data.insert("prefix".to_string(), "ANNOUNCE".to_string());
        MailDataBuilder { data }
    }

    fn emitter(&mut self, emitter: &str) -> &mut Self {
        self.data.insert("emitter".to_string(), emitter.to_string());
        self
    }

    fn recipients<S: AsRef<str>>(&mut self, recipients: &[S]) -> &mut Self {
        let recipients = recipients
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<&str>>()
            .join(", ");
        self.data.insert("recipients".to_string(), recipients);
        self
    }

    fn info(&mut self, info: &ReleaseInfo) -> &mut Self {
        self.data
            .insert("project".to_string(), info.project.clone());
        self.data.insert("url".to_string(), info.url.clone());
        self.data
            .insert("version".to_string(), info.version.clone());
        let text = format!("What's new?\n\n```\n{}```", info.changelog);
        self.data.insert("text".to_string(), text);
        self
    }

    fn signature(&mut self, text: &str) -> &mut Self {
        self.data.insert("signature".to_string(), text.to_string());
        self
    }

    fn extra(&mut self, data: HashMap<String, String>) -> &mut Self {
        self.data.extend(data);
        self
    }

    /// Consume a `MailDataBuilder`
    fn build(self) -> HashMap<String, String> {
        self.data
    }
}

/// Build a mail
struct MailBuilder {
    template: Option<String>,
}

impl MailBuilder {
    fn new() -> Self {
        MailBuilder { template: None }
    }

    fn template(&mut self, template: &str) -> &mut Self {
        self.template = Some(template.to_string());
        self
    }

    fn build(
        self,
        data: &HashMap<String, String>,
    ) -> Result<String, Box<dyn Error>> {
        let template = self
            .template
            .as_ref()
            .map(String::as_str)
            .unwrap_or(DEFAULT_TEMPLATE);
        let mut handlebars = Handlebars::new();
        handlebars.register_escape_fn(no_escape);
        handlebars.register_template_string("t", template)?;
        let text = handlebars.render("t", data)?;
        Ok(text)
    }
}

fn get_logged_user_email() -> Option<String> {
    let username = env::var("USER").or(env::var("USERNAME")).ok()?;
    env::var("HOSTNAME")
        .map(|h| format!("{}@{}", username, h))
        .ok()
}

fn get_user_email() -> Option<String> {
    if let Ok(email) = env::var("DEBEMAIL") {
        let emitter = env::var("DEBFULLNAME")
            .map(|f| format!("{} <{}>", f, email))
            .unwrap_or(email);
        return Some(emitter);
    }
    env::var("EMAIL").ok().or(get_logged_user_email())
}

fn parse_parameter(s: &str) -> Option<(String, String)> {
    let mut split = s.splitn(2, ':');
    if let Some(key) = split.next() {
        if let Some(value) = split.next() {
            return Some((key.to_string(), value.to_string()));
        }
    }
    None
}

fn add_recipients_from_path<P: AsRef<Path>>(
    recipients: &mut Vec<String>,
    path: P,
) -> Result<(), Box<dyn Error>> {
    let input = File::open(path)?;
    let reader = BufReader::new(input);
    for line in reader.lines() {
        let line = line?;
        recipients.push(line);
    }
    Ok(())
}

fn get_signature() -> Option<String> {
    if let Some(mut path) = dirs::home_dir() {
        path.push(".signature");
        let text = fs::read_to_string(&path).ok()?;
        return Some(text);
    }
    None
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut opts = KemennOpts::from_args();
    let emitter = opts
        .emitter
        .or_else(get_user_email)
        .ok_or(format_err!("Missing emitter email"))?;
    if let Some(input) = opts.input {
        add_recipients_from_path(&mut opts.recipients, input)?;
    }
    let mut project = Project::new(&opts.repository);
    if let Some(changelog) = opts.changelog {
        project.set_changelog(&changelog);
    }
    let info = project.latest_release()?;
    let mut builder = MailDataBuilder::new();
    builder
        .emitter(&emitter)
        .recipients(&opts.recipients)
        .info(&info);
    if let Some(signature) = get_signature() {
        builder.signature(&signature);
    }
    if let Some(parameters) = opts.parameters {
        let parameters: HashMap<String, String> = parameters
            .iter()
            .filter_map(|s| parse_parameter(s))
            .collect();
        builder.extra(parameters);
    }
    let data = builder.build();
    let mut builder = MailBuilder::new();
    if let Some(template) = opts.template {
        let text = fs::read_to_string(template)?;
        builder.template(&text);
    }
    let text = builder.build(&data)?;
    if let Some(output) = opts.output {
        fs::write(output, text)?;
    } else {
        print!("{}", text);
    }
    Ok(())
}
