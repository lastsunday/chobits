use sea_orm::DatabaseConnection;

pub mod chobits;
pub mod util;

#[derive(Clone, Debug)]
pub struct AppState {
    pub conn: DatabaseConnection,
}
