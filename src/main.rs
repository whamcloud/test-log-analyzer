use std::path::Path;
use rayon::prelude::*;
mod file_ops;

fn main() -> Result<()>{
    run_analyzer(Path::new("test.dat"));
    Ok(())
}

fn run_analyzer(path: &std::path::Path){
    let mut workers = file_ops::chunk_file(path).unwrap();
    if workers.len() == 1{
        let result = workers.pop().ok_or(anyhow::anyhow!("failed to get worker")).unwrap().analyze();
        println!("{:?}", result);
    }else{
        std::thread::scope(move |scope| {
            let mut results = Vec::with_capacity(workers.len());
            for worker in workers{
                let handle = scope.spawn(move || {
                    worker.analyze()
                });
                results.push(handle);
            }
            println!("{}", results.into_iter()
                .map(|handle| handle.join())
                .filter_map(Result::ok)
                .filter_map(Result::ok)
                .sum::<LogOutput>()
            );
                
        });
        
        //let mut log_output = LogOutput::default();
        //while let Ok(resp) =  rx.recv(){
        //    let _ =  resp
        //        .inspect_err(|e| eprintln!("Worker failed with error {:?}", e))
        //        .map(|output|{
        //            log_output += output;
        //        });

        //}

    }
}

use anyhow::Result;
use std::str::from_utf8;


#[derive(Default, Debug)]
struct LogOutput{
    trace: u64,
    debug: u64,
    info: u64,
    error: u64,
    invalid : u64,
}

impl std::iter::Sum for LogOutput{
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self{
        iter.fold(LogOutput::default(), |mut acc, item|{
            acc += item;
            acc
        })
    }
}
impl std::fmt::Display for LogOutput{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result{
        write!(
            f,
            "Trace: {} \nDebug: {}\n Info: {}\nError: {}\n\nInvalid: {}",
            self.trace,
            self.debug,
            self.info,
            self.error,
            self.invalid
        )
    }
}

impl std::ops::AddAssign for LogOutput{

    fn add_assign(&mut self, other: Self){
        *self = Self{
            trace : self.trace + other.trace,
            debug : self.debug + other.debug,
            info : self.info + other.info,
            error : self.error + other.error,
            invalid : self.invalid + other.invalid
        };
    }
}

impl LogOutput{
    fn inc(&mut self, log_level: Option<LogLevel>){
        match log_level{
            Some(LogLevel::TRACE) => self.trace += 1,
            Some(LogLevel::DEBUG) => self.debug += 1,
            Some(LogLevel::INFO) => self.info += 1,
            Some(LogLevel::ERROR) => self.error += 1,
            None => self.invalid += 1,
        }
    }
}


#[repr(u8)]
enum LogLevel{
    TRACE = 0,
    DEBUG = 1 ,
    INFO = 2,
    ERROR = 3,
}

impl LogLevel{
    fn parse(input: &[u8]) -> Result<Self>{
        match input{
            b"TRACE" => Ok(Self::TRACE),
            b"DEBUG" => Ok(Self::DEBUG),
            b"INFO" => Ok(Self::INFO),
            b"ERROR" => Ok(Self::ERROR),
            u => Err(anyhow::anyhow!("unknown log level {:?}", from_utf8(u)))
        }
    }
}
    

fn parse_log_line(input: &Vec<u8>) -> Result<LogLevel>{
    let mut splitter = input.split(|c| *c == b'|');
    let _ = splitter.next().ok_or(anyhow::anyhow!("failed to read the first date entry"))?;
    splitter
        .next()
        .ok_or(anyhow::anyhow!("no input left in splice"))
        .and_then(|s| LogLevel::parse(s))
    
}
