use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        name: "Linux Voice Flow",
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_has_product_name_and_package_version() {
        let info = get_app_info();
        assert_eq!(info.name, "Linux Voice Flow");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }
}
