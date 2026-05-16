use std::env;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug)]
enum AppError {
    Usage {
        message: String,
    },
    Convert(scel2rime::Error),
    WriteOutput {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage { message } => write!(f, "{message}"),
            Self::Convert(source) => write!(f, "{source}"),
            Self::WriteOutput { path, source } => {
                write!(f, "failed to write {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Convert(source) => Some(source),
            Self::WriteOutput { source, .. } => Some(source),
            Self::Usage { .. } => None,
        }
    }
}

impl From<scel2rime::Error> for AppError {
    fn from(source: scel2rime::Error) -> Self {
        Self::Convert(source)
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(count) => {
            println!("{count} word records are loaded.");
            ExitCode::SUCCESS
        }
        Err(error) => {
            print_usage();
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<usize, AppError> {
    let input = parse_args()?;
    let scel = scel2rime::parse_scel_path(&input)?;
    let output = scel2rime::default_output_path(&input)?;
    let rendered = scel2rime::render_rime_dict(&scel);
    fs::write(&output, rendered).map_err(|source| AppError::WriteOutput {
        path: output,
        source,
    })?;

    Ok(scel.word_list.len())
}

fn parse_args() -> Result<PathBuf, AppError> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 1 {
        return Err(AppError::Usage {
            message: format!("wrong number of arguments: expected 1, got {}", args.len()),
        });
    }

    Ok(PathBuf::from(&args[0]))
}

fn print_usage() {
    eprintln!(
        "{} - {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_DESCRIPTION")
    );
    eprintln!("Usage: {} <file>", env!("CARGO_PKG_NAME"));
}
