use clap::Parser;
use serde::de::Error as SerdeError;
use serde::Deserialize;
use std::process::exit;
use thiserror::Error;

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "kebab-case")]
/// The configuration structure used to define a test case.
pub struct Configuration {
    #[serde(deserialize_with = "deserialize_command")]
    command: (String, Vec<String>),
    stdout: Option<String>,
    #[serde(default)]
    exit_code: i32,
}

fn deserialize_command<'a, D: serde::Deserializer<'a>>(
    d: D,
) -> std::result::Result<(String, Vec<String>), D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Command {
        List(Vec<String>),
        String(String),
    }

    let l = Command::deserialize(d)?;
    match l {
        Command::List(mut ls) if !ls.is_empty() => Ok((ls.remove(0), ls)),
        Command::String(s) if s.trim().contains(' ') => Err(D::Error::custom(
            "Please define a list instead of a string.",
        )),
        Command::String(s) if !s.is_empty() => Ok((s, vec![])),
        _ => Err(D::Error::custom("Command needs at least one element")),
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO")]
    IO(#[from] std::io::Error),
}

fn run(
    config: &Configuration,
    log_file: &mut impl std::io::Write,
) -> std::result::Result<bool, Error> {
    let executable = &config.command.0;
    let process = std::process::Command::new(&executable)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .args(&config.command.1)
        .spawn()?;

    let output = process.wait_with_output()?;
    let output_stdout = String::from_utf8_lossy(&output.stdout);
    let output_status_code = output.status.code();

    let exit_code_failed = match (output_status_code, config.exit_code) {
        (Some(code), expected_exit_code) => code != expected_exit_code,
        (None, _) => true, // killed by signal, currently handled as failure
    };

    let stdout_failed = match (&config.stdout, &output_stdout) {
        (Some(expected_stdout), s) => s != expected_stdout,
        (None, _) => false,
    };

    if exit_code_failed {
        match output_status_code {
            None => writeln!(
                log_file,
                "The process died due to a signal. Expected it to exit with status code {}",
                config.exit_code
            )?,
            Some(exit_code) => writeln!(
                log_file,
                "Unexpected exit code {}, expected {}",
                exit_code, config.exit_code
            )?,
        }
    }

    if stdout_failed {
        writeln!(log_file, "stdout:          {:?}", output_stdout)?;
        if let Some(expected_stdout) = &config.stdout {
            writeln!(log_file, "expected stdout: {:?}", expected_stdout)?;
        } else {
            writeln!(log_file, "expected no stdout.")?;
        }
    }

    let failed = stdout_failed | exit_code_failed;
    if failed {
        writeln!(
            log_file,
            "stdout: {:?}",
            String::from_utf8_lossy(&output.stdout)
        )?;
        writeln!(
            log_file,
            "stderr: {:?}",
            String::from_utf8_lossy(&output.stderr)
        )?;
    }

    Ok(!failed)
}

#[derive(Debug, Parser)]
#[clap(version, author, about)]
pub struct Cli {
    file: String,
}

fn main() {
    let cli = Cli::parse();
    let mut fh = std::fs::File::open(&cli.file).expect("Failed to open the configuration file");
    let config = serde_yaml::from_reader(&mut fh).expect("Failed to parse configuration file");
    match run(&config, &mut std::io::stdout()).unwrap() {
        true => {
            println!("No errors.");
            exit(0)
        }
        false => {
            println!("Errors.");
            exit(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn discard() -> impl std::io::Write {
        pub struct Discard;
        impl std::io::Write for Discard {
            fn write(&mut self, d: &[u8]) -> std::io::Result<usize> {
                Ok(d.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        Discard {}
    }

    fn capture() -> std::io::Cursor<Vec<u8>> {
        let x = std::io::Cursor::new(vec![]);
        x
    }

    #[test]
    fn test_parse_configuration() {
        let config = r#"
command:
  - echo
  - foo

exit-code: 0
stdout: foo
"#;
        let config: Configuration = serde_yaml::from_str(config).unwrap();
        assert_eq!(&config.command.0, "echo");
        assert_eq!(&config.command.1, &["foo"]);
        assert_eq!(config.stdout, Some("foo".to_string()));
        assert_eq!(config.exit_code, 0);
    }

    #[test]
    fn test_parse_configuration_command_single_string() {
        let input = "command: foo bar baz";
        let result: Result<Configuration, _> = serde_yaml::from_str(input);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e
                .to_string()
                .starts_with("Please define a list instead of a string"));
        }
    }

    #[test]
    fn test_parse_configuration_command_empty_string() {
        let input = r#"command: """#;
        let result: Result<Configuration, _> = serde_yaml::from_str(input);
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(
                e.to_string(),
                "Command needs at least one element at line 1 column 8"
            );
        }
    }

    #[test]
    fn test_run_hello_world() {
        let config = Configuration {
            command: (
                "sh".to_string(),
                vec!["-c".to_string(), "exit 1".to_string()],
            ),
            exit_code: 1,
            ..Configuration::default()
        };

        let result = run(&config, &mut discard()).unwrap();
        assert_eq!(result, true);
    }

    #[test]
    fn test_run_exit1() {
        let config = Configuration {
            command: (
                "sh".to_string(),
                vec!["-c".to_string(), "exit 1".to_string()],
            ),
            exit_code: 1,
            ..Configuration::default()
        };
        let result = run(&config, &mut discard()).unwrap();
        assert_eq!(result, true);
    }

    #[test]
    fn test_run_unexpected_exit1() {
        let config = Configuration {
            command: (
                "sh".to_string(),
                vec!["-c".to_string(), "exit 1".to_string()],
            ),
            exit_code: 0,
            ..Configuration::default()
        };
        let result = run(&config, &mut discard()).unwrap();
        assert_eq!(result, false);
    }

    #[test]
    fn test_run_spits_out_stdout_on_exit_mismatch() {
        let config = Configuration {
            command: (
                "sh".to_string(),
                vec!["-c".to_string(), "echo foo bar baz".to_string()],
            ),
            exit_code: 1,
            ..Configuration::default()
        };

        let mut capture = capture();
        let result = run(&config, &mut capture).unwrap();
        assert_eq!(result, false);
        let o = capture.into_inner();
        let output = String::from_utf8_lossy(&o);
        assert!(
            output.contains(r#"stdout: "foo bar baz\n""#),
            "output: {:?}",
            output
        );
    }

    #[test]
    fn test_run_spits_out_stderr_on_exit_mismatch() {
        let config = Configuration {
            command: (
                "sh".to_string(),
                vec!["-c".to_string(), "echo foo bar baz >&2".to_string()],
            ),
            exit_code: 1,
            ..Configuration::default()
        };

        let mut capture = capture();
        let result = run(&config, &mut capture).unwrap();
        assert_eq!(result, false);
        let o = capture.into_inner();
        let output = String::from_utf8_lossy(&o);
        assert!(
            output.contains(r#"stderr: "foo bar baz\n""#),
            "output: {:?}",
            output
        );
    }
}
