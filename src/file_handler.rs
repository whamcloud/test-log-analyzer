use std::{fs::File, path::Path};

use crate::errors::LogAnalyzerErrors;

pub struct FileHandler<'a>(pub &'a String);

impl<'a> FileHandler<'a> {
    pub fn validate(&self) -> Result<(), LogAnalyzerErrors<'_>> {
        if Path::new(&self.0).exists() {
            Ok(())
        } else {
            Err(LogAnalyzerErrors::FileNotFound(&self.0))
        }
    }
    pub fn file_size(&self) -> Result<u64, LogAnalyzerErrors<'_>> {
        let file = File::open(&self.0).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => LogAnalyzerErrors::FileNotFound(&self.0),
            std::io::ErrorKind::PermissionDenied => LogAnalyzerErrors::PermissionDenied(&self.0),
            _ => LogAnalyzerErrors::IoError(self.0, "Unable to fetch file".to_string()),
        })?;

        let metadata = file.metadata().map_err(|_| {
            LogAnalyzerErrors::IoError(&self.0, "Unable to read metadata".to_string())
        })?;
        Ok(metadata.len())
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_validate_existing_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "test data").unwrap();

        let path = file.path().to_str().unwrap().to_string();
        let handler = FileHandler(&path);

        assert!(handler.validate().is_ok());
    }

    #[test]
    fn test_validate_non_existent_file() {
        let path = "non_existent_file_12345.log".to_string();
        let handler = FileHandler(&path);

        assert!(handler.validate().is_err());
    }

    #[test]
    fn test_file_size_calculation() {
        let mut file = NamedTempFile::new().unwrap();
        let content = "Hello Rust";
        file.write_all(content.as_bytes()).unwrap();

        let path = file.path().to_str().unwrap().to_string();
        let handler = FileHandler(&path);

        assert_eq!(handler.file_size().unwrap(), content.len() as u64);
    }
}
