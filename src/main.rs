use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const OUTPUT_DIR: &str = "dist";

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
    CreateOutputDir {
        path: PathBuf,
        source: std::io::Error,
    },
    HttpRequest {
        url: String,
        source: Box<ureq::Error>,
    },
    ReadResponse {
        url: String,
        source: Box<ureq::Error>,
    },
    ResolveDownloadUrl {
        id: u32,
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
            Self::CreateOutputDir { path, source } => {
                write!(f, "failed to create {}: {source}", path.display())
            }
            Self::HttpRequest { url, source } => write!(f, "failed HTTP request {url}: {source}"),
            Self::ReadResponse { url, source } => {
                write!(f, "failed to read HTTP response {url}: {source}")
            }
            Self::ResolveDownloadUrl { id } => {
                write!(
                    f,
                    "failed to resolve Sogou download URL for dictionary id {id}"
                )
            }
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
            Self::CreateOutputDir { source, .. } | Self::WriteOutput { source, .. } => Some(source),
            Self::HttpRequest { source, .. } | Self::ReadResponse { source, .. } => {
                Some(source.as_ref())
            }
            Self::Usage { .. } | Self::ResolveDownloadUrl { .. } => None,
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
    let output_dir = PathBuf::from(OUTPUT_DIR);
    fs::create_dir_all(&output_dir).map_err(|source| AppError::CreateOutputDir {
        path: output_dir.clone(),
        source,
    })?;

    let mut total_words = 0usize;
    let mut converted = 0usize;

    for source in &config.dictionaries {
        let buffer = download_scel(source)?;
        let scel = scel2rime::parse_scel_bytes(&buffer, source.id.to_string())?;
        let output = scel2rime::output_path_for_source(&output_dir, source);
        write_rime_dict(&output, &scel)?;

        println!(
            "converted Sogou {} ({}) -> {} ({} words)",
            source.id,
            source_label(source),
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

fn download_scel(source: &scel2rime::ScelSource) -> Result<Vec<u8>, AppError> {
    let (url, _name) = resolve_download(source)?;
    read_url_bytes(&url)
}

fn resolve_download(source: &scel2rime::ScelSource) -> Result<(String, String), AppError> {
    if !source.name.is_empty() {
        return Ok((scel2rime::sogou_download_url(source), source.name.clone()));
    }

    let detail_url = scel2rime::sogou_detail_url(source.id);
    let detail = read_url_bytes(&detail_url)?;
    let html = String::from_utf8_lossy(&detail);
    let name = resolve_name_from_detail_html(source.id, &html)
        .ok_or(AppError::ResolveDownloadUrl { id: source.id })?;
    Ok((
        scel2rime::sogou_download_url_with_name(source.id, &name),
        name,
    ))
}

fn read_url_bytes(url: &str) -> Result<Vec<u8>, AppError> {
    let mut response = ureq::get(url)
        .call()
        .map_err(|source| AppError::HttpRequest {
            url: url.to_string(),
            source: Box::new(source),
        })?;

    response
        .body_mut()
        .read_to_vec()
        .map_err(|source| AppError::ReadResponse {
            url: url.to_string(),
            source: Box::new(source),
        })
}

fn resolve_name_from_detail_html(id: u32, html: &str) -> Option<String> {
    let marker = format!("download_cell.php?id={id}&name=");
    let start = html.find(&marker)? + marker.len();
    let rest = &html[start..];
    let end = rest.find("&f=detail")?;
    Some(rest[..end].to_string())
}

fn source_label(source: &scel2rime::ScelSource) -> &str {
    if source.name.is_empty() {
        "id-only config"
    } else {
        &source.name
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
