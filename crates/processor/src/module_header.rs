//! Bounded public-module metadata shared by the runtime and embedded catalog builder.
use std::{
    fs::File,
    io::{self, BufRead, BufReader, Read},
    path::Path,
};

pub const MAX_CDN_FILE_BYTES: u64 = 20_000_000;

pub fn read(path: &Path) -> io::Result<String> {
    const LIMIT: u64 = 1024 * 1024;
    let mut reader = BufReader::new(File::open(path)?.take(LIMIT + 1));
    let mut header = String::new();
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        if header.len() + line.len() > LIMIT as usize {
            return Err(io::Error::other(
                "Public module metadata exceeds its size limit",
            ));
        }
        if line.trim_start().starts_with('[') {
            break;
        }
        header.push_str(&line);
    }
    Ok(header)
}
