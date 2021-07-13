use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

struct Cli {
    path: PathBuf,
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

fn main() -> std::io::Result<()> {
    let path = env::args().nth(1).expect("no path given");
    let args = Cli {
        path: PathBuf::from(path),
    };
    let buffer = fs::read(&args.path).unwrap();
    let pre = format!(
        "# Rime Sogou
# encoding: utf-8
#
# name: {name}
# type: {category}
# example: {example}
#
---
name: luna_pinyin.sogou.{filename}
version: \"{version}\"
sort: by_weight
use_preset_vocabulary: true
...
    ",
        name = read_string(&buffer, 0x130, 0x338),
        category = read_string(&buffer, 0x338, 0x540),
        example = read_string(&buffer, 0xd40, 0x1540),
        filename = args.path.as_path().file_stem().unwrap().to_string_lossy(),
        version = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    println!("{}", pre);
    let mut pos: usize = 0x1544;
    let mut pinyin_lst: Vec<String> = Vec::new();
    while pos + 4 <= 0x2628 {
        let pinyin_len = u16::from_le_bytes([buffer[pos + 2], buffer[pos + 3]]) as usize;
        pos += 4;
        pinyin_lst.push(read_string(&buffer, pos, pos + pinyin_len));
        pos += pinyin_len;
    }
    let mut pos: usize = 0x2628;
    let mut word_lst: Vec<String> = Vec::new();
    let content = 'outer: loop {
        let word_count = u16::from_le_bytes([buffer[pos], buffer[pos + 1]]);
        pos += 2;
        let py_count_len = u16::from_le_bytes([buffer[pos], buffer[pos + 1]]);
        pos += 2;
        let mut pinyins: String = String::new();
        for idx in 0..py_count_len / 2 {
            let py_idx = u16::from_le_bytes([buffer[pos], buffer[pos + 1]]) as usize;
            if py_idx >= pinyin_lst.len() {
                break 'outer word_lst.join("\n");
            }
            let py = &pinyin_lst[py_idx];
            if idx != 0 {
                pinyins.push_str(" ")
            };
            pinyins.push_str(&py);
            pos += 2;
        }
        for _ in 0..word_count {
            let word_len = u16::from_le_bytes([buffer[pos], buffer[pos + 1]]) as usize;
            pos += 2;
            let word = read_string(&buffer, pos, pos + word_len);
            pos += word_len;
            let ext_len = u16::from_le_bytes([buffer[pos], buffer[pos + 1]]) as usize;
            pos += 2;
            let w = format!(
                "{}\t{}\t{}",
                word,
                &pinyins,
                u32::from_le_bytes([
                    buffer[pos],
                    buffer[pos + 1],
                    buffer[pos + 2],
                    buffer[pos + 3],
                ])
            );
            pos += ext_len;
            word_lst.push(w);
        }
    };
    fs::write(
        format!(
            "./luna_pinyin.sogou.{}.dict.yaml",
            args.path.as_path().file_stem().unwrap().to_string_lossy()
        ),
        format!("{}\n{}", pre, content),
    )?;
    Ok(())
}
