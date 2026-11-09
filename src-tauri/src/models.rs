//! Read-only discovery of structurally complete, installed catalogue models.
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

const CATALOG: &str = include_str!("../../config/models.json");
const ASSETS: [&str; 4] = [
    "model.bin",
    "config.json",
    "tokenizer.json",
    "vocabulary.txt",
];
#[derive(Debug, Clone, Serialize)]
pub struct LocalModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub size_bytes: u64,
    pub is_default: bool,
}
#[derive(Debug)]
pub enum CatalogError {
    InvalidCatalog,
    Unavailable,
}
#[derive(Deserialize)]
struct Catalog {
    schema_version: u32,
    default_model_id: String,
    models: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    id: String,
    name: String,
    description: String,
}
fn catalog(source: &str) -> Result<Catalog, CatalogError> {
    let catalog: Catalog =
        serde_json::from_str(source).map_err(|_| CatalogError::InvalidCatalog)?;
    if catalog.schema_version != 1 || !matches!(catalog.default_model_id.as_str(), "tiny" | "base")
    {
        return Err(CatalogError::InvalidCatalog);
    }
    let mut ids = std::collections::HashSet::new();
    for entry in &catalog.models {
        if !matches!(entry.id.as_str(), "tiny" | "base") || !ids.insert(&entry.id) {
            return Err(CatalogError::InvalidCatalog);
        }
    }
    if !ids.contains(&catalog.default_model_id) {
        return Err(CatalogError::InvalidCatalog);
    }
    Ok(catalog)
}
// Reject symlinks in every existing path component, including root ancestors.
fn real_directory(path: &Path) -> Result<(), CatalogError> {
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor).map_err(|_| CatalogError::Unavailable)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(CatalogError::Unavailable);
        }
    }
    Ok(())
}
pub fn installed_models(root: &Path) -> Result<Vec<LocalModelInfo>, CatalogError> {
    let catalog = catalog(CATALOG)?;
    real_directory(root)?;
    let root = root.canonicalize().map_err(|_| CatalogError::Unavailable)?;
    let models = root.join("models");
    match fs::symlink_metadata(&models) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(CatalogError::Unavailable),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(CatalogError::Unavailable)
        }
        Ok(_) => {}
    }
    // Surface inaccessible discovery roots even when none of the known IDs exist.
    fs::read_dir(&models).map_err(|_| CatalogError::Unavailable)?;
    let mut installed = Vec::new();
    for entry in catalog.models {
        let path = models.join(&entry.id);
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            continue;
        }
        let canonical = path.canonicalize().map_err(|_| CatalogError::Unavailable)?;
        if canonical != path || !canonical.starts_with(&models) {
            continue;
        }
        let mut size_bytes = 0u64;
        let mut complete = true;
        for asset in ASSETS {
            match fs::symlink_metadata(path.join(asset)) {
                Ok(metadata)
                    if metadata.is_file()
                        && !metadata.file_type().is_symlink()
                        && metadata.len() > 0 =>
                {
                    size_bytes = size_bytes
                        .checked_add(metadata.len())
                        .ok_or(CatalogError::Unavailable)?;
                }
                _ => {
                    complete = false;
                    break;
                }
            }
        }
        if complete {
            installed.push(LocalModelInfo {
                is_default: entry.id == catalog.default_model_id,
                id: entry.id,
                name: entry.name,
                description: entry.description,
                path: canonical.to_string_lossy().into_owned(),
                size_bytes,
            });
        }
    }
    Ok(installed)
}
pub fn installed_model_id(root: &Path, path: &Path) -> Option<String> {
    installed_models(root)
        .ok()?
        .into_iter()
        .find(|entry| Path::new(&entry.path) == path)
        .map(|entry| entry.id)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn install(root: &Path, id: &str) {
        let path = root.join("models").join(id);
        fs::create_dir_all(&path).unwrap();
        for asset in ASSETS {
            fs::write(path.join(asset), b"fake").unwrap();
        }
    }
    #[test]
    fn discovery_only_returns_complete_whitelisted_models() {
        let root = tempfile::tempdir().unwrap();
        assert!(installed_models(root.path()).unwrap().is_empty());
        install(root.path(), "tiny");
        install(root.path(), "base");
        install(root.path(), "unknown");
        let entries = installed_models(root.path()).unwrap();
        assert_eq!(
            entries.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["tiny", "base"]
        );
        assert_eq!(entries[0].size_bytes, 16);
        assert!(entries[0].is_default);
        fs::write(root.path().join("models/base/model.bin"), b"").unwrap();
        assert_eq!(installed_models(root.path()).unwrap().len(), 1);
    }
    #[test]
    fn unsafe_catalog_ids_are_rejected() {
        for id in ["../tiny", "/tiny", "tiny/base", "unknown"] {
            let source = serde_json::json!({"schema_version":1,"default_model_id":"tiny","models":[{"id":id,"name":"test","description":"test"}]}).to_string();
            assert!(catalog(&source).is_err());
        }
        assert!(catalog(CATALOG).is_ok());
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_assets_directories_and_ancestors_are_rejected() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        install(root.path(), "tiny");
        let alias = root.path().join("alias");
        symlink(root.path(), &alias).unwrap();
        assert!(installed_models(&alias).is_err());
        let weight = root.path().join("models/tiny/model.bin");
        fs::remove_file(&weight).unwrap();
        symlink(root.path().join("models/tiny/config.json"), weight).unwrap();
        assert!(installed_models(root.path()).unwrap().is_empty());
        symlink(
            root.path().join("models/tiny"),
            root.path().join("models/base"),
        )
        .unwrap();
        assert!(installed_models(root.path()).unwrap().is_empty());
    }
    #[test]
    fn selection_identity_matches_installed_paths_only() {
        let root = tempfile::tempdir().unwrap();
        install(root.path(), "tiny");
        install(root.path(), "base");
        for id in ["tiny", "base"] {
            assert_eq!(
                installed_model_id(root.path(), &root.path().join("models").join(id)),
                Some(id.into())
            );
        }
        assert_eq!(
            installed_model_id(root.path(), &root.path().join("custom")),
            None
        );
    }
}
