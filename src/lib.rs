#![cfg_attr(feature = "ci", deny(warnings))]

extern crate serde_json;
extern crate tracetree;

pub mod error;

use self::error::AppResult;
use serde_json::Value;
use std::process::Command;
use tracetree::ProcessTree;

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
