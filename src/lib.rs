#![cfg_attr(feature = "ci", deny(warnings))]

#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
extern crate serde_json;

pub mod error;
mod tracetree;

use self::error::AppResult;
use self::tracetree::ProcessTree;
use serde_json::Value;
use std::process::Command;

pub fn trace(path: &str) -> AppResult<String> {
    let args: Vec<String> = vec![];
    let process_tree = ProcessTree::spawn(Command::new(path), &args)?;
    let string = serde_json::to_string_pretty(&process_tree)?;
    let result: Value = serde_json::from_str(&string)?;
    Ok(result
        .get("children")
        .ok_or("no field children")?
        .get(0)
        .ok_or("no child")?
        .get("cmdline")
        .ok_or("no field cmdline")?
        .get(0)
        .ok_or("no cmdline entries")?
        .as_str()
        .ok_or("not a string")?
        .to_string())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn returns_the_first_called_child_process() {
        assert_eq!(trace("./test/test-01.sh"), Ok("/bin/true".to_string()));
    }
}
