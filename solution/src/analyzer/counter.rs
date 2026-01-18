#[derive(Default, Debug, PartialEq)]
pub struct Counts {
    pub info: u64,
    pub warn: u64,
    pub error: u64,
    pub malformed: u64,
}
