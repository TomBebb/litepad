use std::fmt;
use std::fs::File;
use std::io::{BufWriter, BufRead, BufReader, Write};
use std::path::PathBuf;

/// A source from which documents can be loaded
#[derive(Clone, Eq, PartialEq)]
pub enum Source {
    Unknown,
    File(PathBuf),
}

impl Source {
    pub fn load(&self) -> String {
        let mut text = String::new();
        if let Some(mut reader) = self.reader() {
            reader.read_to_string(&mut text).unwrap();
        }
        text
    }
    pub fn writer(&self) -> Option<Box<Write>> {
        match *self {
            Source::File(ref path) => {
                let path = path.as_path();
                let file = File::create(path).unwrap();
                Some(Box::new(BufWriter::new(file)))
            }
            _ => None,
        }
    }
    pub fn reader(&self) -> Option<Box<BufRead>> {
        match *self {
            Source::File(ref path) => {
                let path = path.as_path();
                let file = File::open(path).unwrap();
                Some(Box::new(BufReader::new(file)))
            }
            _ => None,
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Source::Unknown => f.write_str("Untitled"),
            Source::File(ref path) => path.display().fmt(f),
        }
    }
}
