use std::env;

use test_log_analyzer::{
    config::Config,
    errors::LogAnalyzerErrors,
    file_handler::FileHandler,
    log_processor::{LogProcessor, ParallelLogProcessor, SequentialLogProcessor},
};

const MIN_PARALLEL_FILE_SIZE: u64 = 104_857_600; // 100 MB

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        LogAnalyzerErrors::LogFileNotProvided.out();
        std::process::exit(1);
    }

    let log_file_path = &args[1];

    let file_handler = FileHandler(log_file_path);

    if let Err(e) = file_handler.validate() {
        e.out();
        std::process::exit(1);
    }

    let cfg = match args.get(2) {
        Some(custom_file) => match Config::read_from_file(custom_file) {
            Ok(cfg) => cfg,
            Err(e) => {
                e.out();
                std::process::exit(1);
            }
        },
        None => Config::default(),
    };

    let file_size = match file_handler.file_size() {
        Ok(size) => size,
        Err(e) => {
            e.out();
            std::process::exit(1);
        }
    };

    let execute_parallel = cfg.parallel.unwrap_or(true) && file_size >= MIN_PARALLEL_FILE_SIZE;

    let processor: Box<dyn LogProcessor + '_> = if execute_parallel {
        Box::new(ParallelLogProcessor {
            file_path: log_file_path,
            cfg: &cfg,
        })
    } else {
        Box::new(SequentialLogProcessor {
            file_path: log_file_path,
            cfg: &cfg,
        })
    };

    match processor.process() {
        Ok(summary) => {
            summary.print();
        }
        Err(e) => {
            e.out();
            std::process::exit(1);
        }
    }
}
