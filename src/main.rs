use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

struct Cli {
    path: PathBuf,
}

struct WordRecord {
    word: String,
    pinyin: String,
    frequency: u16,
}

struct Scel {
    word_list: Vec<WordRecord>,
    example: String,
    version: String,
    id: String,
    name: String,
    category: String,
    scel_length: String,
    file_name: String,
}

fn read_string(buffer: &[u8], start: usize, end: usize) -> String {
    let rtn: Vec<u16> = buffer[start..end]
        .chunks_exact(2)
        .into_iter()
        .map(|a| u16::from_le_bytes([a[0], a[1]]))
        .collect();
    String::from_utf16_lossy(&rtn)
        .trim_end_matches(char::from(0))
        .replace("\u{3000}", "")
        .replace("\r", " ")
        .into()
}

fn parse_scel_file(fp: &PathBuf) -> Scel {
    let buffer = fs::read(fp).unwrap();
    assert!(
        &buffer[0x000..=0x000B] == b"\x40\x15\x00\x00\x44\x43\x53\x01\x01\x00\x00\x00",
        "This is not a scel file of Sogou!"
    );

    let scel_id = read_string(&buffer, 0x001C, 0x011B);
    let scel_length = u16::from_le_bytes([buffer[0x124], buffer[0x125]]);
    let name = read_string(&buffer, 0x130, 0x338);
    let category = read_string(&buffer, 0x338, 0x540);
    let example = read_string(&buffer, 0xd40, 0x1540);
    let filename = fp
        .as_path()
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert!(
        &buffer[0x1540..=0x1543] == b"\x9D\x01\x00\x00",
        "This is not a scel file of Sogou!"
    );

    let mut pos: usize = 0x1544;
    let mut pinyin_lst: Vec<String> = Vec::new();
    while u16::from_le_bytes([buffer[pos], buffer[pos + 1]]) == pinyin_lst.len() as u16 {
        let pinyin_len = u16::from_le_bytes([buffer[pos + 2], buffer[pos + 3]]) as usize;
        pos += 4;
        pinyin_lst.push(read_string(&buffer, pos, pos + pinyin_len));
        pos += pinyin_len;
    }

    assert!(
        pos == 0x2628,
        "The starting position of word table is different！！！"
    );
    let mut word_list: Vec<WordRecord> = Vec::new();
    // 一个词条最少需要 22 个字节
    // word_count: 2
    // py_count_len: 2
    // py: 2
    // word_len: 2
    // word: 2
    // extension: 12
    while pos + 22 <= buffer.len() {
        let word_count = u16::from_le_bytes([buffer[pos], buffer[pos + 1]]) as u8;
        pos += 2;
        let py_count_len = u16::from_le_bytes([buffer[pos], buffer[pos + 1]]) as usize;
        pos += 2;
        if py_count_len % 2 != 0 {
            println!("Found a word that takes odd number of bytes!!!");
            break;
        }
        let pinyin: String = buffer[pos..=pos + py_count_len]
            .chunks_exact(2)
            .into_iter()
            .map(|a| u16::from_le_bytes([a[0], a[1]]))
            .map(|a| pinyin_lst[a as usize].as_ref())
            .collect::<Vec<&str>>()
            .join(" ");
        pos += py_count_len;
        for _ in 0..word_count {
            if u16::from_le_bytes([buffer[pos], buffer[pos + 1]]) != py_count_len as u16 {
                println!("The word length is not equal to pinyin length!");
                break;
            }
            pos += 2;
            let word = read_string(&buffer, pos, pos + py_count_len);
            pos += py_count_len;
            assert!(
                u16::from_le_bytes([buffer[pos], buffer[pos + 1]]) == 10,
                "The extension length is not equal to 10 bytes!"
            );
            let freq = u16::from_le_bytes([buffer[pos + 2], buffer[pos + 3]]);
            word_list.push(WordRecord {
                word,
                pinyin: pinyin.clone(),
                frequency: freq,
            });
            pos += 12;
        }
    }
    Scel {
        name,
        word_list,
        id: scel_id,
        scel_length: scel_length.to_string(),
        example,
        category,
        version: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string(),
        file_name: filename,
    }
}

fn main() -> std::io::Result<()> {
    let path = env::args().nth(1).expect("no path given");
    let args = Cli {
        path: PathBuf::from(path),
    };
    let scel = parse_scel_file(&args.path);
    let pre = format!(
        "# Rime Sogou
# encoding: utf-8
#
# 名字: {}
# ID: {}
# 类型: {}
# 例子: {}
# 词条数目: {}
---
name: luna_pinyin.sogou.{}
version: \"{}\"
sort: by_weight
use_preset_vocabulary: true
...
    ",
        scel.name,
        scel.id,
        scel.category,
        scel.example,
        scel.scel_length,
        scel.file_name,
        scel.version
    );
    println!("{}", pre);
    fs::write(
        format!("./luna_pinyin.sogou.{}.dict.yaml", scel.file_name),
        format!(
            "{}\n{}",
            pre,
            scel.word_list
                .iter()
                .map(|w| format!("{}\t{}\t{}", w.word, w.pinyin, w.frequency))
                .collect::<Vec<String>>()
                .join("\n"),
        ),
    )?;
    println!("{} word records are loaded!!!", scel.scel_length);
    Ok(())
}
