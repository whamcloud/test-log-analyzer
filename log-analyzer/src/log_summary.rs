#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LogSummary {
    pub info: u64,
    pub warn: u64,
    pub error: u64,
    pub malformed: u64,
}

impl LogSummary {
    pub fn total(&self) -> u64 {
        self.info + self.warn + self.error + self.malformed
    }

    pub fn display(&self) {
        println!("================================");
        println!("|  Log Analysis Summary         |");
        println!("|===============================|");
        println!("| INFO:      {:>18} |", self.info);
        println!("| WARN:      {:>18} |", self.warn);
        println!("| ERROR:     {:>18} |", self.error);
        println!("| MALFORMED: {:>18} |", self.malformed);
        println!("|================================");
        println!("\nTotal lines: {}", self.total());
    }
}
