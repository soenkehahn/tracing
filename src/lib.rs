#![cfg_attr(feature = "ci", deny(warnings))]

#[macro_use]
extern crate log;
extern crate serde_json;

pub mod error;
mod tracetree;

use self::error::AppResult;
use self::tracetree::ProcessTree;
use std::process::Command;

pub fn trace(path: &str) -> AppResult<String> {
    let args: Vec<String> = vec![];
    let process_tree = ProcessTree::spawn(Command::new(path), &args)?;
    let descendants = process_tree.get_descendants();
    Ok(descendants[0].clone())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn returns_the_first_called_child_process() {
        assert_eq!(trace("./test/test-01.sh"), Ok("/bin/true".to_string()));
    }
}
