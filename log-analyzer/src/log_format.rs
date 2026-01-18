use crate::error::LogError;

#[derive(Debug, Clone, Copy)]
pub struct LogFormat {
    pub delimiter: char,
    pub level_position: usize,
}

impl LogFormat {
    pub fn standard() -> Self {
        Self {
            delimiter: '|',
            level_position: 1,
        }
    }

    pub fn space_delimited() -> Self {
        Self {
            delimiter: ' ',
            level_position: 1,
        }
    }

    pub fn csv_delimited() -> Self {
        Self {
            delimiter: ',',
            level_position: 1,
        }
    }

    pub fn custom(delimiter: char, level_position: usize) -> Self {
        Self {
            delimiter,
            level_position,
        }
    }

    pub fn parse_level<'a>(&self, line: &'a str) -> Result<&'a str, LogError> {
        let mut parts = line.split(self.delimiter);

        let level = parts.nth(self.level_position).ok_or_else(|| {
            LogError::InvalidFormat(format!("Missing field at position {}", self.level_position))
        })?;
        Ok(level.trim())
    }
}
