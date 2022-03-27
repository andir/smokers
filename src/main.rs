use serde::Deserialize;
use thiserror::Error;
use std::process::exit;
use clap::Parser;
use serde::de::Error as SerdeError;

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all="kebab-case")]
/// The configuration structure used to define a test case.
pub struct Configuration {
    #[serde(deserialize_with="deserialize_command")]
    command: (String, Vec<String>),
    stdout: Option<String>,
    #[serde(default)]
    exit_code: i32,
}

fn deserialize_command<'a, D: serde::Deserializer<'a>>(d: D) -> std::result::Result<(String, Vec<String>), D::Error> {

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Command {
	List(Vec<String>),
	String(String),
    }
    
    let l = Command::deserialize(d)?;
    match l {
	Command::List(mut ls) if !ls.is_empty() => Ok((ls.remove(0), ls)),
	Command::String(s) if s.trim().contains(' ') => {
	    Err(D::Error::custom("Please define a list instead of a string."))
	}
	Command::String(s) if !s.is_empty() => {
	    let mut parts : Vec<_> = s.split(' ').map(String::from).collect();
	    Ok((parts.remove(0), parts))
	}
	_ => {
	    Err(D::Error::custom("Command needs at least one element"))
	}
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO")]
    IO(#[from] std::io::Error)
}

fn run(config: &Configuration) -> std::result::Result<bool, Error> {
    let executable = &config.command.0;
    let process = std::process::Command::new(&executable)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
	.args(&config.command.1)
	.spawn()?;

    let output = process.wait_with_output()?;

    let failed = match output.status.code() {
        Some(code) if code == config.exit_code => {
    	    false
        }
        Some(_) | None /* None = killed by signal */ => {
            println!("Wrong or unexpected exit code {:?}. Expected {:?}", output.status.code(), config.exit_code);
    	    true
        }
    };

    let failed = failed | if let Some(expected_stdout) = &config.stdout {
	let output_string = String::from_utf8_lossy(&output.stdout);
	if !output_string.eq(expected_stdout) {
	    println!("Got unexpected stdout output.");
	    println!("expected: {:?}", expected_stdout);
	    println!("got     : {:?}", output_string);
	    true
	} else {
	    false
	}
    } else { false };

    Ok(!failed)
}


#[derive(Debug, Parser)]
#[clap(version,author,about)]
pub struct Cli {
    file: String,
}

fn main() {
    let cli = Cli::parse();
    let mut fh = std::fs::File::open(&cli.file).expect("Failed to open the configuration file");
    let config = serde_yaml::from_reader(&mut fh).expect("Failed to parse configuration file");
    match run(&config).unwrap() {
	true => {
	    println!("No errors.");
	    exit(0)
	},
	false => {
	    println!("Errors.");
	    exit(1)
	},
    }
}
