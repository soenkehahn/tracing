use tracing::trace;

fn main() {
    let executable = std::env::args().into_iter().nth(1).unwrap();
    let first_child_process = trace(&executable);
    println!("first spawned child process: {}", first_child_process);
}
