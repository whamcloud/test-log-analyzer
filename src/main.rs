use std::path::PathBuf;
use anyhow::Result;
mod file_ops;
mod types;

use types::LogOutput;

fn main() -> Result<()>{
    let res = run_analyzer(PathBuf::from("test.dat"))?;
    println!("{}", res);
    Ok(())
}

fn run_analyzer(path: std::path::PathBuf) -> Result<LogOutput>{
    //let num = std::thread::available_parallelism()?;
    let path = std::sync::Arc::new(path);
    let core_ids = core_affinity::get_core_ids().ok_or(anyhow::anyhow!("couldnt get core id"))?;
    let num = core_ids.len() * 4;
    let mut workers = file_ops::chunk_file(&path, num).unwrap();
    if workers.len() == 1{
        workers.pop().ok_or(anyhow::anyhow!("failed to get worker")).unwrap().analyze()
    }else{
        let mut result = Ok(LogOutput::default());
        let point = &mut result;

        let (tx, rx) = crossbeam_channel::unbounded();
        for worker in workers{
            tx.send(worker)?;
        }
        drop(tx);
        std::thread::scope(move |scope| {
            let mut handles = Vec::with_capacity(core_ids.len());
            for core_id in core_ids{
                let rx = rx.clone();
                let handle = scope.spawn(move || {
                    core_affinity::set_for_current(core_id);
                    let mut local_log_output = LogOutput::default();

                    while let Ok(worker) = rx.recv(){
                        if let Ok(res)  = worker.analyze(){
                            local_log_output += res;
                        }
                    }
                    return local_log_output;
                });
                handles.push(handle);
            }
            *point = Ok(handles.into_iter()
                        .map(|handle| handle.join())
                        .filter_map(Result::ok)
                        .sum()
            );
        });
      result
    }
}

