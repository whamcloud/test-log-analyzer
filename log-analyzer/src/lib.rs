pub mod error;
pub mod log_format;
pub mod log_summary;
pub mod processor;

#[macro_export]
macro_rules! time_block {
    ($name:literal, $block:block) => {{
        let start = ::std::time::Instant::now();
        let result = { $block };
        let elapsed = start.elapsed();
        println!("{} completed in {:.3?}", $name, elapsed);
        result
    }};
}
