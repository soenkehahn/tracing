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

bless!(tracetree::Error);
bless!(&str);
bless!(serde_json::Error);
