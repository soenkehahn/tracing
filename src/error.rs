use std::fmt::Debug;

pub type AppResult<A> = Result<A, AppError>;

#[derive(Debug, PartialEq)]
pub struct AppError(String);

macro_rules! bless {
    ( $error_type:ty ) => {
        impl From<$error_type> for AppError {
            fn from(error: $error_type) -> AppError {
                AppError(format!("{:?}", error))
            }
        }
    };
}

bless!(&str);
bless!(serde_json::Error);
bless!(std::io::Error);
bless!(nix::Error);
bless!(String);

pub trait ChainErr {
    type Result;

    fn chain_err(self, message: fn() -> &'static str) -> AppResult<Self::Result>;
}

impl<A, Error> ChainErr for Result<A, Error>
where
    Error: Debug,
{
    type Result = A;

    fn chain_err(self, message: fn() -> &'static str) -> AppResult<Self::Result> {
        match self {
            Ok(a) => Ok(a),
            Err(error) => Err(AppError(format!("{}: {:?}", message(), error))),
        }
    }
}

pub fn bail<A>(message: String) -> AppResult<A> {
    Err(AppError(message))
}
