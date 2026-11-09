fn main() {
    #[cfg(feature = "desktop")]
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_app_info",
            "get_hotkey_status",
            "replace_hotkey",
            "get_dictation_status",
            "list_microphones",
            "list_local_models",
            "configure_dictation",
            "cancel_dictation",
            "clear_dictation_result",
            "copy_dictation_result",
            "clear_dictation_clipboard",
            "get_settings",
            "save_settings",
            "get_diagnostics",
        ]),
    ))
    .expect("failed to prepare desktop application");
}
