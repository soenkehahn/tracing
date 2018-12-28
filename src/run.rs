use crate::error::AppResult;
use crate::trace;

pub fn run() -> AppResult<()> {
    let executable = std::env::args()
        .nth(1)
        .ok_or("please provide an executable as argument")?;
    let child_processes = trace(executable)?;
    print!("{}", format(child_processes));
    Ok(())
}

pub fn format(processes: Vec<String>) -> String {
    let mut result = "spawned child processes:\n".to_string();
    for process in processes.into_iter() {
        result.push_str(&format!("  {}\n", process));
    }
    result
}

#[cfg(test)]
mod test {
    use super::*;

    mod format {
        use super::*;

        #[test]
        fn format_works() {
            let input = vec!["foo".to_string(), "bar".to_string()];
            assert_eq!(format(input), "spawned child processes:\n  foo\n  bar\n");
        }
    }
}
