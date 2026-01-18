pub mod counter;
mod parser;

use counter::Counts;
use parser::process_line;
use std::io::{self, BufRead};

pub fn analyze<R: BufRead>(mut reader: R) -> io::Result<Counts> {
    let mut counts = Counts::default();
    let mut buffer = Vec::with_capacity(1024);

    loop {
        buffer.clear();
        let n = reader.read_until(b'\n', &mut buffer)?;
        if n == 0 {
            break;
        }
        process_line(&buffer, &mut counts);
    }

    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_and_invalid_lines() {
        let input = b"\
t|INFO|s|ok
t|WARN|s|warn
bad
";

        let counts = analyze(&input[..]).unwrap();
        assert_eq!(counts.info, 1);
        assert_eq!(counts.warn, 1);
        assert_eq!(counts.malformed, 1);
    }
}
