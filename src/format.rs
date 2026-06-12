/// Describes how a log line is structured.
///
/// Holds the byte delimiter and the 0-indexed position of the level field.
/// `Copy` so it can be passed freely into closures and across threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogFormat {
    pub delimiter: u8,
    pub level_position: usize,
}

impl LogFormat {
    /// `timestamp|level|service|message`
    pub fn standard() -> Self {
        Self { delimiter: b'|', level_position: 1 }
    }

    /// `timestamp level service message`
    pub fn space() -> Self {
        Self { delimiter: b' ', level_position: 1 }
    }

    /// `timestamp,level,service,message`
    pub fn csv() -> Self {
        Self { delimiter: b',', level_position: 1 }
    }

    pub fn custom(delimiter: u8, level_position: usize) -> Self {
        Self { delimiter, level_position }
    }

    /// Parse a `--delimiter` CLI argument (single ASCII character).
    pub fn parse_delimiter(s: &str) -> Option<u8> {
        let mut chars = s.chars();
        let c = chars.next()?;
        if chars.next().is_some() { return None; } // more than one char
        if c.is_ascii() { Some(c as u8) } else { None }
    }
}

impl Default for LogFormat {
    fn default() -> Self {
        Self::standard()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_defaults() {
        let f = LogFormat::standard();
        assert_eq!(f.delimiter, b'|');
        assert_eq!(f.level_position, 1);
    }

    #[test]
    fn parse_delimiter_single_ascii() {
        assert_eq!(LogFormat::parse_delimiter("|"), Some(b'|'));
        assert_eq!(LogFormat::parse_delimiter(","), Some(b','));
        assert_eq!(LogFormat::parse_delimiter(" "), Some(b' '));
    }

    #[test]
    fn parse_delimiter_rejects_multi_char() {
        assert_eq!(LogFormat::parse_delimiter("||"), None);
        assert_eq!(LogFormat::parse_delimiter(""), None);
    }
}
