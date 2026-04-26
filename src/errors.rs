#[derive(Debug)]
pub enum LogAnalyzerErrors<'a> {
    LogFileNotProvided,
    FileNotFound(&'a String),
    ConfigReadError(String, &'a String),
    PermissionDenied(&'a str),
    IoError(&'a str, String),
}

impl<'a> LogAnalyzerErrors<'a> {
    pub fn out(&self) {
        println!("======");
        println!("ERROR:");
        match self {
            LogAnalyzerErrors::LogFileNotProvided => {
                println!("Log file path not provided, please provide file path")
            }
            LogAnalyzerErrors::FileNotFound(file) => {
                println!("File Does Not Exists | path: `{}`", file)
            }
            LogAnalyzerErrors::ConfigReadError(msg, file) => {
                println!("ConfigFileError: Error[{}] path:`{}`", msg, file)
            }
            LogAnalyzerErrors::PermissionDenied(file) => {
                println!(
                    "Permission denied: You do not have access to '{}'. Try running with elevated privileges.",
                    file
                )
            }
            LogAnalyzerErrors::IoError(file, error) => {
                println!(
                    "I/O Error: An unexpected system error occurred while accessing '{}'. [ERROR]{}",
                    file, error
                )
            }
        };
        println!("======");
    }
}
