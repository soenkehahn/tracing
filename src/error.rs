pub type AppResult<A> = Result<A, AppError>;

#[derive(Debug, PartialEq)]
pub struct AppError(String);

impl From<&str> for AppError {
    fn from(error: &str) -> AppError {
        AppError(format!("{:?}", error))
    }
}
