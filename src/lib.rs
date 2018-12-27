extern crate serde_json;
extern crate tracetree;

use serde_json::Value;
use std::process::Command;
use tracetree::ProcessTree;

pub fn trace(path: &str) -> String {
    let args: Vec<String> = vec![];
    let string =
        serde_json::to_string_pretty(&ProcessTree::spawn(Command::new(path), &args).unwrap())
            .unwrap();
    let result: Value = serde_json::from_str(&string).unwrap();
    let first = result
        .get("children")
        .unwrap()
        .get(0)
        .unwrap()
        .get("cmdline")
        .unwrap()
        .get(0)
        .unwrap()
        .as_str();
    first.unwrap().to_string()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn returns_the_first_called_child_process() {
        assert_eq!(trace("./test/test-01.sh"), "/bin/true");
    }
}
