use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitCode};

const CACHE_DIR: &str = ".scel2rime-cache";

#[derive(Debug)]
enum CliCommand {
    ConvertFile(PathBuf),
    ConvertConfig(PathBuf),
}

#[derive(Debug)]
enum AppError {
    Usage {
        message: String,
    },
    Convert(scel2rime::Error),
    CreateCacheDir {
        path: PathBuf,
        source: std::io::Error,
    },
    ReadDownload {
        path: PathBuf,
        source: std::io::Error,
    },
    DownloadSpawn {
        source: std::io::Error,
    },
    DownloadFailed {
        source: scel2rime::ScelSource,
        stderr: String,
    },
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
            Self::CreateCacheDir { path, source } => {
                write!(f, "failed to create {}: {source}", path.display())
            }
            Self::ReadDownload { path, source } => {
                write!(f, "failed to read downloaded {}: {source}", path.display())
            }
            Self::DownloadSpawn { source } => write!(f, "failed to run curl: {source}"),
            Self::DownloadFailed { source, stderr } => write!(
                f,
                "failed to download Sogou dictionary {} ({}): {}",
                source.id,
                source.name,
                stderr.trim()
            ),
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
            Self::CreateCacheDir { source, .. }
            | Self::ReadDownload { source, .. }
            | Self::DownloadSpawn { source }
            | Self::WriteOutput { source, .. } => Some(source),
            Self::Usage { .. } | Self::DownloadFailed { .. } => None,
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
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            print_usage();
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<String, AppError> {
    match parse_args()? {
        CliCommand::ConvertFile(input) => convert_file(&input),
        CliCommand::ConvertConfig(config_path) => convert_config(&config_path),
    }
}

fn convert_file(input: &Path) -> Result<String, AppError> {
    let scel = scel2rime::parse_scel_path(input)?;
    let output = scel2rime::default_output_path(input)?;
    write_rime_dict(&output, &scel)?;

    Ok(format!(
        "converted {} word records to {}",
        scel.word_list.len(),
        output.display()
    ))
}

fn convert_config(config_path: &Path) -> Result<String, AppError> {
    let config = scel2rime::parse_config_path(config_path)?;
    let cache_dir = PathBuf::from(CACHE_DIR);
    fs::create_dir_all(&cache_dir).map_err(|source| AppError::CreateCacheDir {
        path: cache_dir.clone(),
        source,
    })?;

    let mut total_words = 0usize;
    let mut converted = 0usize;

    for source in &config.dictionaries {
        let download_path = scel2rime::downloaded_scel_path(&cache_dir, source);
        download_scel(source, &download_path)?;

        let buffer = fs::read(&download_path).map_err(|source| AppError::ReadDownload {
            path: download_path.clone(),
            source,
        })?;
        let scel = scel2rime::parse_scel_bytes(&buffer, source.id.to_string())?;
        let output = scel2rime::output_path_for_source(source);
        write_rime_dict(&output, &scel)?;

        println!(
            "converted Sogou {} ({}) -> {} ({} words)",
            source.id,
            source.name,
            output.display(),
            scel.word_list.len()
        );
        total_words += scel.word_list.len();
        converted += 1;
    }

    Ok(format!(
        "converted {converted} dictionaries with {total_words} total word records"
    ))
}

fn write_rime_dict(output: &Path, scel: &scel2rime::Scel) -> Result<(), AppError> {
    fs::write(output, scel2rime::render_rime_dict(scel)).map_err(|source| AppError::WriteOutput {
        path: output.to_path_buf(),
        source,
    })
}

fn download_scel(source: &scel2rime::ScelSource, output: &Path) -> Result<(), AppError> {
    let url = scel2rime::sogou_download_url(source);
    let command_output = ProcessCommand::new("curl")
        .arg("-L")
        .arg("--fail")
        .arg("--retry")
        .arg("3")
        .arg("--retry-all-errors")
        .arg("--connect-timeout")
        .arg("20")
        .arg("--output")
        .arg(output)
        .arg(url)
        .output()
        .map_err(|source| AppError::DownloadSpawn { source })?;

    if command_output.status.success() {
        Ok(())
    } else {
        Err(AppError::DownloadFailed {
            source: source.clone(),
            stderr: String::from_utf8_lossy(&command_output.stderr).to_string(),
        })
    }
}

fn parse_args() -> Result<CliCommand, AppError> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [path] => Ok(CliCommand::ConvertFile(PathBuf::from(path))),
        [flag, path] if flag == "--config" || flag == "-c" => {
            Ok(CliCommand::ConvertConfig(PathBuf::from(path)))
        }
        _ => Err(AppError::Usage {
            message: format!("wrong arguments: {}", format_args_for_error(&args)),
        }),
    }
}

fn format_args_for_error(args: &[OsString]) -> String {
    if args.is_empty() {
        "no arguments".to_string()
    } else {
        args.iter()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn print_usage() {
    eprintln!(
        "{} - {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_DESCRIPTION")
    );
    eprintln!("Usage: {} <file.scel>", env!("CARGO_PKG_NAME"));
    eprintln!("Usage: {} --config <config-file>", env!("CARGO_PKG_NAME"));
}
