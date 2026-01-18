use solution::analyzer::analyze;
use solution::io::reader::open_reader;

fn main() -> std::io::Result<()> {
    let path = std::env::args().nth(1);
    let reader = open_reader(path)?;
    let counts = analyze(reader)?;

    println!("INFO: {}", counts.info);
    println!("WARN: {}", counts.warn);
    println!("ERROR: {}", counts.error);
    println!("MALFORMED: {}", counts.malformed);

    Ok(())
}
