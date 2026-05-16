use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const HEADER_LEN: usize = 12;
const PINYIN_TABLE_COUNT_OFFSET: usize = 0x1540;
const PINYIN_TABLE_OFFSET: usize = 0x1544;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordRecord {
    pub word: String,
    pub pinyin: String,
    pub frequency: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scel {
    pub word_list: Vec<WordRecord>,
    pub example: String,
    pub version: String,
    pub id: String,
    pub name: String,
    pub category: String,
    pub file_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScelConfig {
    pub dictionaries: Vec<ScelSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScelSource {
    pub id: u32,
    pub name: String,
}

#[derive(Debug)]
pub enum Error {
    ReadFile {
        path: PathBuf,
        source: io::Error,
    },
    MissingFileStem {
        path: PathBuf,
    },
    InvalidHeader {
        found: Vec<u8>,
    },
    UnexpectedEof {
        section: &'static str,
        offset: usize,
        needed: usize,
        len: usize,
    },
    OddByteLength {
        section: &'static str,
        offset: usize,
        len: usize,
    },
    InvalidPinyinIndex {
        offset: usize,
        index: usize,
        pinyin_count: usize,
    },
    InvalidPinyinTableIndex {
        offset: usize,
        expected: usize,
        actual: usize,
    },
    InvalidExtensionLength {
        offset: usize,
        len: usize,
    },
    InvalidConfigLine {
        line_number: usize,
        line: String,
        message: String,
    },
    EmptyConfig,
    SystemTime(std::time::SystemTimeError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadFile { path, source } => {
                write!(f, "failed to read {}: {source}", path.display())
            }
            Self::MissingFileStem { path } => {
                write!(f, "input path has no file stem: {}", path.display())
            }
            Self::InvalidHeader { found } => write!(
                f,
                "invalid Sogou SCEL header: expected DCS or ECS signature, found {:02x?}",
                found
            ),
            Self::UnexpectedEof {
                section,
                offset,
                needed,
                len,
            } => write!(
                f,
                "unexpected EOF in {section} at offset 0x{offset:x}: need {needed} bytes, file has {len} bytes"
            ),
            Self::OddByteLength {
                section,
                offset,
                len,
            } => write!(
                f,
                "odd UTF-16 byte length in {section} at offset 0x{offset:x}: {len} bytes"
            ),
            Self::InvalidPinyinIndex {
                offset,
                index,
                pinyin_count,
            } => write!(
                f,
                "invalid pinyin index {index} at offset 0x{offset:x}; pinyin table has {pinyin_count} entries"
            ),
            Self::InvalidPinyinTableIndex {
                offset,
                expected,
                actual,
            } => write!(
                f,
                "invalid pinyin table index at offset 0x{offset:x}: expected {expected}, got {actual}"
            ),
            Self::InvalidExtensionLength { offset, len } => write!(
                f,
                "invalid word extension length at offset 0x{offset:x}: {len} bytes"
            ),
            Self::InvalidConfigLine {
                line_number,
                line,
                message,
            } => write!(
                f,
                "invalid config line {line_number}: {message}; line: {line}"
            ),
            Self::EmptyConfig => write!(f, "config has no Sogou dictionaries"),
            Self::SystemTime(source) => write!(f, "failed to calculate unix timestamp: {source}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadFile { source, .. } => Some(source),
            Self::SystemTime(source) => Some(source),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn parse_config_path(path: impl AsRef<Path>) -> Result<ScelConfig> {
    let path = path.as_ref();
    let contents = fs::read_to_string(path).map_err(|source| Error::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    parse_config_str(&contents)
}

pub fn parse_config_str(contents: &str) -> Result<ScelConfig> {
    let mut dictionaries = Vec::new();

    for (index, raw_line) in contents.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line
            .split_once('#')
            .map_or(raw_line, |(content, _comment)| content)
            .trim();

        if line.is_empty() {
            continue;
        }

        let (id, name) = parse_config_entry(line_number, line)?;
        dictionaries.push(ScelSource { id, name });
    }

    if dictionaries.is_empty() {
        return Err(Error::EmptyConfig);
    }

    Ok(ScelConfig { dictionaries })
}

pub fn sogou_detail_url(id: u32) -> String {
    format!("https://pinyin.sogou.com/dict/detail/index/{id}")
}

pub fn sogou_download_url(source: &ScelSource) -> String {
    sogou_download_url_with_name(source.id, &source.name)
}

pub fn sogou_download_url_with_name(id: u32, name: &str) -> String {
    format!(
        "https://pinyin.sogou.com/d/dict/download_cell.php?id={id}&name={}&f=detail",
        percent_encode(name)
    )
}

pub fn output_path_for_source(output_dir: impl AsRef<Path>, source: &ScelSource) -> PathBuf {
    output_dir
        .as_ref()
        .join(format!("luna_pinyin.sogou.{}.dict.yaml", source.id))
}

pub fn parse_scel_path(path: impl AsRef<Path>) -> Result<Scel> {
    let path = path.as_ref();
    let buffer = fs::read(path).map_err(|source| Error::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let file_name = path
        .file_stem()
        .ok_or_else(|| Error::MissingFileStem {
            path: path.to_path_buf(),
        })?
        .to_string_lossy()
        .to_string();

    parse_scel_bytes(&buffer, file_name)
}

pub fn parse_scel_bytes(buffer: &[u8], file_name: impl Into<String>) -> Result<Scel> {
    validate_header(buffer)?;

    let id = read_utf16_string(buffer, 0x001c, 0x011c, "dictionary id")?;
    let name = read_utf16_string(buffer, 0x0130, 0x0338, "dictionary name")?;
    let category = read_utf16_string(buffer, 0x0338, 0x0540, "dictionary category")?;
    let example = read_utf16_string(buffer, 0x0d40, 0x1540, "dictionary examples")?;
    let expected_word_count = read_u32(buffer, 0x0124, "dictionary word count")? as usize;

    let (pinyin_list, mut pos) = parse_pinyin_table(buffer)?;
    let mut word_list = Vec::new();

    while pos + 4 <= buffer.len()
        && (expected_word_count == 0 || word_list.len() < expected_word_count)
    {
        let word_count = read_u16(buffer, pos, "word count")? as usize;
        pos += 2;
        let pinyin_bytes_len = read_u16(buffer, pos, "word pinyin length")? as usize;
        pos += 2;

        if pinyin_bytes_len == 0 && word_count == 0 {
            break;
        }
        if !pinyin_bytes_len.is_multiple_of(2) {
            return Err(Error::OddByteLength {
                section: "word pinyin indices",
                offset: pos,
                len: pinyin_bytes_len,
            });
        }

        let pinyin_index_bytes = read_bytes(buffer, pos, pinyin_bytes_len, "word pinyin indices")?;
        let pinyin = pinyin_index_bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]) as usize)
            .map(|index| {
                pinyin_list
                    .get(index)
                    .ok_or(Error::InvalidPinyinIndex {
                        offset: pos,
                        index,
                        pinyin_count: pinyin_list.len(),
                    })
                    .map(String::as_str)
            })
            .collect::<Result<Vec<_>>>()?
            .join(" ");
        pos += pinyin_bytes_len;

        for _ in 0..word_count {
            let word_len = read_u16(buffer, pos, "word length")? as usize;
            pos += 2;
            let word = read_utf16_string_at(buffer, pos, word_len, "word")?;
            pos += word_len;

            let extension_len = read_u16(buffer, pos, "word extension length")? as usize;
            if extension_len < 2 {
                return Err(Error::InvalidExtensionLength {
                    offset: pos,
                    len: extension_len,
                });
            }
            let frequency = read_u16(buffer, pos + 2, "word frequency")?;
            read_bytes(buffer, pos + 2, extension_len, "word extension")?;
            pos += 2 + extension_len;

            word_list.push(WordRecord {
                word,
                pinyin: pinyin.clone(),
                frequency,
            });
        }
    }

    Ok(Scel {
        name,
        word_list,
        id,
        example,
        category,
        version: unix_timestamp()?,
        file_name: file_name.into(),
    })
}

pub fn default_output_path(input: &Path) -> Result<PathBuf> {
    let stem = input
        .file_stem()
        .ok_or_else(|| Error::MissingFileStem {
            path: input.to_path_buf(),
        })?
        .to_string_lossy();
    Ok(PathBuf::from(format!("luna_pinyin.sogou.{stem}.dict.yaml")))
}

pub fn render_rime_dict(scel: &Scel) -> String {
    let header = format!(
        "# Rime Sogou\n\
# encoding: utf-8\n\
#\n\
# 名字: {}\n\
# ID: {}\n\
# 类型: {}\n\
# 例子: {}\n\
# 词条数目: {}\n\
---\n\
name: luna_pinyin.sogou.{}\n\
version: \"{}\"\n\
sort: by_weight\n\
use_preset_vocabulary: true\n\
...\n",
        scel.name,
        scel.id,
        scel.category,
        scel.example,
        scel.word_list.len(),
        scel.file_name,
        scel.version
    );

    let entries = scel
        .word_list
        .iter()
        .map(|record| format!("{}\t{}\t{}", record.word, record.pinyin, record.frequency))
        .collect::<Vec<_>>()
        .join("\n");

    if entries.is_empty() {
        header
    } else {
        format!("{header}\n{entries}\n")
    }
}

fn parse_config_entry(line_number: usize, line: &str) -> Result<(u32, String)> {
    let (id_part, name_part) = if let Some((id, name)) = line.split_once('=') {
        (id.trim(), name.trim())
    } else {
        let mut parts = line.splitn(2, char::is_whitespace);
        let id = parts.next().unwrap_or_default().trim();
        let name = parts.next().unwrap_or_default().trim();
        (id, name)
    };

    if id_part.is_empty() {
        return Err(Error::InvalidConfigLine {
            line_number,
            line: line.to_string(),
            message: "missing dictionary id".to_string(),
        });
    }
    let id = id_part
        .parse::<u32>()
        .map_err(|source| Error::InvalidConfigLine {
            line_number,
            line: line.to_string(),
            message: format!("invalid dictionary id: {source}"),
        })?;

    Ok((id, name_part.to_string()))
}

fn percent_encode(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .flat_map(|byte| match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                vec![*byte as char]
            }
            byte => format!("%{byte:02X}").chars().collect::<Vec<_>>(),
        })
        .collect()
}

fn validate_header(buffer: &[u8]) -> Result<()> {
    let header = read_bytes(buffer, 0, HEADER_LEN, "SCEL header")?;
    let valid = header[0..4] == [0x40, 0x15, 0x00, 0x00]
        && (&header[4..7] == b"DCS" || &header[4..7] == b"ECS")
        && header[7..12] == [0x01, 0x01, 0x00, 0x00, 0x00];

    if valid {
        Ok(())
    } else {
        Err(Error::InvalidHeader {
            found: header.to_vec(),
        })
    }
}

fn parse_pinyin_table(buffer: &[u8]) -> Result<(Vec<String>, usize)> {
    let pinyin_count = read_u16(buffer, PINYIN_TABLE_COUNT_OFFSET, "pinyin table count")? as usize;
    let mut pinyin_list = Vec::with_capacity(pinyin_count);
    let mut pos = PINYIN_TABLE_OFFSET;

    for expected in 0..pinyin_count {
        let actual = read_u16(buffer, pos, "pinyin table index")? as usize;
        if actual != expected {
            return Err(Error::InvalidPinyinTableIndex {
                offset: pos,
                expected,
                actual,
            });
        }
        let len = read_u16(buffer, pos + 2, "pinyin table entry length")? as usize;
        pos += 4;
        let pinyin = read_utf16_string_at(buffer, pos, len, "pinyin table entry")?;
        pinyin_list.push(pinyin);
        pos += len;
    }

    Ok((pinyin_list, pos))
}

fn read_utf16_string(
    buffer: &[u8],
    start: usize,
    end: usize,
    section: &'static str,
) -> Result<String> {
    let len = end.saturating_sub(start);
    read_utf16_string_at(buffer, start, len, section)
}

fn read_utf16_string_at(
    buffer: &[u8],
    offset: usize,
    len: usize,
    section: &'static str,
) -> Result<String> {
    if !len.is_multiple_of(2) {
        return Err(Error::OddByteLength {
            section,
            offset,
            len,
        });
    }
    let bytes = read_bytes(buffer, offset, len, section)?;
    let units = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();

    Ok(String::from_utf16_lossy(&units)
        .chars()
        .take_while(|ch| *ch != '\0')
        .collect::<String>()
        .replace('\u{3000}', "")
        .replace('\r', " ")
        .trim()
        .to_string())
}

fn read_u16(buffer: &[u8], offset: usize, section: &'static str) -> Result<u16> {
    let bytes = read_bytes(buffer, offset, 2, section)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(buffer: &[u8], offset: usize, section: &'static str) -> Result<u32> {
    let bytes = read_bytes(buffer, offset, 4, section)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_bytes<'a>(
    buffer: &'a [u8],
    offset: usize,
    len: usize,
    section: &'static str,
) -> Result<&'a [u8]> {
    if offset.checked_add(len).is_none_or(|end| end > buffer.len()) {
        return Err(Error::UnexpectedEof {
            section,
            offset,
            needed: len,
            len: buffer.len(),
        });
    }
    Ok(&buffer[offset..offset + len])
}

fn unix_timestamp() -> Result<String> {
    Ok(SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(Error::SystemTime)?
        .as_secs()
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_ecs_dictionary() {
        let buffer = sample_scel(b"ECS");

        let scel = parse_scel_bytes(&buffer, "sample").expect("sample should parse");

        assert_eq!(scel.id, "4");
        assert_eq!(scel.name, "网络流行新词");
        assert_eq!(scel.category, "北京");
        assert_eq!(scel.example, "与凤行（影剧名词）");
        assert_eq!(scel.file_name, "sample");
        assert_eq!(scel.word_list.len(), 1);
        assert_eq!(
            scel.word_list[0],
            WordRecord {
                word: "你好".to_string(),
                pinyin: "ni hao".to_string(),
                frequency: 123,
            }
        );
    }

    #[test]
    fn parses_legacy_dcs_header() {
        let buffer = sample_scel(b"DCS");

        let scel = parse_scel_bytes(&buffer, "sample").expect("sample should parse");

        assert_eq!(scel.word_list[0].pinyin, "ni hao");
    }

    #[test]
    fn rejects_invalid_header() {
        let mut buffer = sample_scel(b"ECS");
        buffer[4] = b'X';

        let error = parse_scel_bytes(&buffer, "sample").expect_err("header should fail");

        assert!(matches!(error, Error::InvalidHeader { .. }));
    }

    #[test]
    fn renders_rime_dictionary() {
        let scel = Scel {
            word_list: vec![WordRecord {
                word: "你好".to_string(),
                pinyin: "ni hao".to_string(),
                frequency: 123,
            }],
            example: "例子".to_string(),
            version: "42".to_string(),
            id: "4".to_string(),
            name: "网络流行新词".to_string(),
            category: "北京".to_string(),
            file_name: "sample".to_string(),
        };

        let rendered = render_rime_dict(&scel);

        assert!(rendered.contains("name: luna_pinyin.sogou.sample"));
        assert!(rendered.contains("version: \"42\""));
        assert!(rendered.contains("你好\tni hao\t123"));
    }

    #[test]
    fn parses_id_focused_config() {
        let config = parse_config_str(
            "# Sogou dictionaries\n\
             4 网络流行新词\n\
             5 = 常用诗词\n\
             77212\n",
        )
        .expect("config should parse");

        assert_eq!(
            config.dictionaries,
            vec![
                ScelSource {
                    id: 4,
                    name: "网络流行新词".to_string(),
                },
                ScelSource {
                    id: 5,
                    name: "常用诗词".to_string(),
                },
                ScelSource {
                    id: 77212,
                    name: "".to_string(),
                },
            ]
        );
    }

    #[test]
    fn builds_sogou_download_url() {
        let source = ScelSource {
            id: 4,
            name: "网络流行新词".to_string(),
        };

        assert_eq!(
            sogou_detail_url(4),
            "https://pinyin.sogou.com/dict/detail/index/4"
        );
        assert_eq!(
            sogou_download_url(&source),
            "https://pinyin.sogou.com/d/dict/download_cell.php?id=4&name=%E7%BD%91%E7%BB%9C%E6%B5%81%E8%A1%8C%E6%96%B0%E8%AF%8D&f=detail"
        );
        assert_eq!(
            output_path_for_source("dist", &source),
            PathBuf::from("dist/luna_pinyin.sogou.4.dict.yaml")
        );
    }

    fn sample_scel(signature: &[u8; 3]) -> Vec<u8> {
        let mut buffer = vec![0; PINYIN_TABLE_OFFSET];
        buffer[0..4].copy_from_slice(&[0x40, 0x15, 0x00, 0x00]);
        buffer[4..7].copy_from_slice(signature);
        buffer[7..12].copy_from_slice(&[0x01, 0x01, 0x00, 0x00, 0x00]);
        write_utf16(&mut buffer, 0x001c, "4");
        write_utf16(&mut buffer, 0x0130, "网络流行新词");
        write_utf16(&mut buffer, 0x0338, "北京");
        write_utf16(&mut buffer, 0x0d40, "与凤行（影剧名词）");
        write_u32(&mut buffer, 0x0124, 1);
        write_u16(&mut buffer, PINYIN_TABLE_COUNT_OFFSET, 2);

        push_pinyin(&mut buffer, 0, "ni");
        push_pinyin(&mut buffer, 1, "hao");

        push_u16(&mut buffer, 1); // word count
        push_u16(&mut buffer, 4); // pinyin index byte length
        push_u16(&mut buffer, 0);
        push_u16(&mut buffer, 1);
        push_utf16_with_len(&mut buffer, "你好");
        push_u16(&mut buffer, 10); // extension length
        push_u16(&mut buffer, 123); // frequency
        buffer.extend_from_slice(&[0; 8]);

        buffer
    }

    fn push_pinyin(buffer: &mut Vec<u8>, index: u16, pinyin: &str) {
        push_u16(buffer, index);
        push_utf16_with_len(buffer, pinyin);
    }

    fn push_utf16_with_len(buffer: &mut Vec<u8>, value: &str) {
        let bytes = utf16_bytes(value);
        push_u16(buffer, bytes.len() as u16);
        buffer.extend(bytes);
    }

    fn write_utf16(buffer: &mut [u8], offset: usize, value: &str) {
        let bytes = utf16_bytes(value);
        buffer[offset..offset + bytes.len()].copy_from_slice(&bytes);
    }

    fn write_u16(buffer: &mut [u8], offset: usize, value: u16) {
        buffer[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u32(buffer: &mut [u8], offset: usize, value: u32) {
        buffer[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn push_u16(buffer: &mut Vec<u8>, value: u16) {
        buffer.extend_from_slice(&value.to_le_bytes());
    }

    fn utf16_bytes(value: &str) -> Vec<u8> {
        value
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    }
}
