use std::fs::File;
use std::io::{self, BufRead, BufReader};

pub fn open_reader(path: Option<String>) -> io::Result<Box<dyn BufRead>> {
    Ok(match path {
        Some(p) => Box::new(BufReader::new(File::open(p)?)),
        None => Box::new(BufReader::new(io::stdin())),
    })
}
