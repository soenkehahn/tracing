use crate::error::AppResult;
use crate::trace;
use crate::tracetree::ProcessChild;

pub fn run() -> AppResult<()> {
    let executable = std::env::args()
        .nth(1)
        .ok_or("please provide an executable as argument")?;
    let child_processes = trace(executable)?;
    print!("{}", format(child_processes));
    Ok(())
}

pub fn format(commands: Vec<ProcessChild>) -> String {
    let mut result = "spawned child processes:\n".to_string();
    for command in commands.into_iter() {
        let mut formatted_arguments = "".to_string();
        for argument in command.arguments {
            formatted_arguments += &format!(" {}", argument);
        }
        result.push_str(&format!(
            "  {}{}\n",
            command.executable, formatted_arguments
        ));
    }
    result
}

#[cfg(test)]
mod test {
    use super::*;

    mod format {
        use super::*;

        #[test]
        fn outputs_executables() {
            let input = vec![
                ProcessChild {
                    executable: "foo".to_string(),
                    arguments: vec![],
                },
                ProcessChild {
                    executable: "bar".to_string(),
                    arguments: vec![],
                },
            ];
            assert_eq!(format(input), "spawned child processes:\n  foo\n  bar\n");
        }

        #[test]
        fn outputs_arguments() {
            let input = vec![ProcessChild {
                executable: "foo".to_string(),
                arguments: vec!["bar".to_string(), "baz".to_string()],
            }];
            assert_eq!(format(input), "spawned child processes:\n  foo bar baz\n");
        }
    }
}
