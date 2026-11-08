#[derive(Debug, Clone)]
pub struct Migration {
    pub version: i64,
    pub description: &'static str,
    pub sql: &'static str,
}
