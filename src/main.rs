#![cfg_attr(feature = "ci", deny(warnings))]

use tracing::error::AppResult;
use tracing::trace;

fn main() -> AppResult<()> {
    let executable = std::env::args().nth(1).unwrap();
    let first_child_process = trace(&executable)?;
    println!("first spawned child process: {}", first_child_process);
    Ok(())
}
