pub mod analyzer;
pub mod parser;
pub mod format;

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
