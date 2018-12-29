#![cfg_attr(feature = "ci", deny(warnings))]

use tracing::error::AppResult;
use tracing::run::run;

fn main() -> AppResult<()> {
    run()
}
