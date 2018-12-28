pub type AppResult<A> = Result<A, AppError>;

#[derive(Debug, PartialEq)]
pub struct AppError(String);
