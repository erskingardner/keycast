pub mod database;
pub mod v2;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BUILD_REVISION: &str = match option_env!("KEYCAST_BUILD_REVISION") {
    Some(revision) => revision,
    None => "development",
};
