#![cfg_attr(feature = "ci", deny(warnings))]

#[macro_use]
extern crate log;

pub mod error;
pub mod run;
mod tracetree;

use crate::error::AppResult;
use crate::tracetree::{ProcessChild, ProcessTree};
use std::process::Command;

pub fn trace<Path: AsRef<str>>(path: Path) -> AppResult<Vec<ProcessChild>> {
    let process_tree = ProcessTree::spawn(Command::new(path.as_ref()))?;
    let descendants = process_tree.get_descendants();
    Ok(descendants)
}

#[cfg(test)]
mod test {
    use super::*;

    mod trace {
        use super::*;
        use std::fs::File;
        use std::io::Write;
        use tempfile::TempDir;

        struct TestScript {
            temp_dir: TempDir,
        }

        impl TestScript {
            fn new(contents: &str) -> TestScript {
                let test_script = TestScript {
                    temp_dir: TempDir::new().unwrap(),
                };
                let mut script_file = File::create(test_script.path()).unwrap();
                write!(script_file, "{}", contents).unwrap();
                Command::new("chmod")
                    .arg("+x")
                    .arg(test_script.path())
                    .spawn()
                    .unwrap()
                    .wait()
                    .unwrap();
                test_script
            }

            fn path(&self) -> String {
                format!("{}/script.sh", self.temp_dir.path().to_str().unwrap())
            }
        }

        #[test]
        fn returns_a_called_child_process() {
            let script = TestScript::new("/bin/true;");
            match trace(script.path()).unwrap().as_slice() {
                [ProcessChild { executable, .. }] => assert_eq!(executable, "/bin/true"),
                _ => panic!(),
            }
        }

        #[test]
        fn returns_multiple_called_child_processes() {
            let script = TestScript::new("/bin/true; /bin/false;");
            match trace(script.path()).unwrap().as_slice() {
                [ProcessChild { executable: a, .. }, ProcessChild { executable: b, .. }] => {
                    assert_eq!(a, "/bin/true");
                    assert_eq!(b, "/bin/false");
                }
                _ => panic!(),
            }
        }

        #[test]
        fn includes_process_arguments() {
            let script = TestScript::new("/bin/echo foo;");
            match trace(script.path()).unwrap().as_slice() {
                [ProcessChild { arguments, .. }] => match arguments.as_slice() {
                    [argument] => assert_eq!(argument, "foo"),
                    _ => panic!(),
                },
                _ => panic!(),
            }
        }
    }
}
