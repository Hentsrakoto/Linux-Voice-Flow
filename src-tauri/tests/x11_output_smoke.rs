//! Opt-in native smoke: synthetic text only, on a dedicated Xvfb server.
use linux_voice_flow::insertion::{DesktopOutput, X11Output};
use std::{path::PathBuf, process::{Child, Command, Stdio}, thread, time::{Duration, Instant}};

struct Fixture(Child);
impl Drop for Fixture {
    fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); }
}

#[test]
#[ignore = "requires isolated Xvfb :198, GTK/WebKit Python and explicit opt-in"]
fn unicode_insertion_into_editor_and_browser_without_submitting() {
    assert_eq!(std::env::var("LVF_ISOLATED_X11").as_deref(), Ok("1"));
    assert_eq!(std::env::var("DISPLAY").as_deref(), Ok(":198"));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_owned();
    let temporary = tempfile::tempdir().unwrap();
    let text = "Bonjour élève, café — Unicode 🐢";
    for kind in ["entry", "web"] {
        let state = temporary.path().join(format!("{kind}.json"));
        let _fixture = Fixture(Command::new("/usr/bin/python3")
            .arg(root.join("tests/manual/x11_fixture.py")).arg(kind).arg(&state)
            .env("WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS", "1")
            .env("WEBKIT_DISABLE_COMPOSITING_MODE", "1")
            .stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap());
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let ready = std::fs::read(&state).ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .is_some_and(|state| state["ready"] == true);
            if ready { break; }
            assert!(Instant::now() < deadline, "isolated {kind} fixture did not become ready");
            thread::sleep(Duration::from_millis(30));
        }
        let mut output = X11Output::new(root.clone());
        let target = output.capture_target().unwrap();
        output.insert(&target, text, &|| false).unwrap();
        loop {
            let actual: serde_json::Value = serde_json::from_slice(&std::fs::read(&state).unwrap()).unwrap();
            if actual["text"] == text { break; }
            assert!(Instant::now() < deadline, "synthetic Unicode text differs in {kind}: {actual}");
            thread::sleep(Duration::from_millis(30));
        }
    }
}
