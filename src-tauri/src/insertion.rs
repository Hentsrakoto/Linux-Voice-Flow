//! Local X11 output. Dictated text travels over stdin, never a command line.
pub use crate::focus::FocusTarget;
use crate::{focus, protocol::ErrorCode};
use serde::Serialize;
use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroize;

const TOOL_TIMEOUT: Duration = Duration::from_millis(700);
const INSERT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_TEXT_BYTES: usize = 64 * 1024;
const CHUNK_CHARACTERS: usize = 16;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct OutputDiagnostics {
    pub x11: bool,
    pub xdotool_available: bool,
    pub xclip_available: bool,
}

pub trait DesktopOutput: Send {
    fn capture_target(&mut self) -> Result<FocusTarget, ErrorCode>;
    fn insert(
        &mut self,
        target: &FocusTarget,
        text: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), ErrorCode>;
    fn copy(&mut self, text: &str) -> Result<(), ErrorCode>;
    fn clear_clipboard(&mut self) -> Result<(), ErrorCode>;
    fn diagnostics(&self) -> OutputDiagnostics;
}

pub struct NoopOutput;
impl DesktopOutput for NoopOutput {
    fn capture_target(&mut self) -> Result<FocusTarget, ErrorCode> {
        Err(ErrorCode::FocusChanged)
    }
    fn insert(&mut self, _: &FocusTarget, _: &str, _: &dyn Fn() -> bool) -> Result<(), ErrorCode> {
        Err(ErrorCode::InjectionFailed)
    }
    fn copy(&mut self, _: &str) -> Result<(), ErrorCode> {
        Err(ErrorCode::ClipboardFailed)
    }
    fn clear_clipboard(&mut self) -> Result<(), ErrorCode> {
        Ok(())
    }
    fn diagnostics(&self) -> OutputDiagnostics {
        OutputDiagnostics {
            x11: false,
            xdotool_available: false,
            xclip_available: false,
        }
    }
}

pub struct X11Output {
    xdotool: Option<PathBuf>,
    xclip: Option<PathBuf>,
    x11: bool,
    clipboard_owner: Option<HelperChild>,
}

fn executable(path: &Path) -> Option<PathBuf> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return None;
    }
    path.canonicalize().ok().filter(|value| value.is_absolute())
}

fn command(path: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(path);
    command.args(args).stderr(Stdio::null()).process_group(0);
    command
}

struct HelperChild {
    child: Child,
    reaped: bool,
}

impl HelperChild {
    fn new(child: Child) -> Self {
        Self {
            child,
            reaped: false,
        }
    }

    fn id(&self) -> u32 {
        self.child.id()
    }

    fn exit_status(&self) -> std::io::Result<Option<bool>> {
        // Observe exit without reaping: the leader PID remains reserved until
        // its group is stopped, so we cannot signal a recycled process group.
        let pid = rustix::process::Pid::from_raw(self.id() as i32).expect("spawned helper PID");
        rustix::process::waitid(
            rustix::process::WaitId::Pid(pid),
            rustix::process::WaitIdOptions::EXITED
                | rustix::process::WaitIdOptions::NOHANG
                | rustix::process::WaitIdOptions::NOWAIT,
        )
        .map(|status| status.map(|status| status.exit_status() == Some(0)))
        .map_err(std::io::Error::from)
    }
}

impl Drop for HelperChild {
    fn drop(&mut self) {
        terminate(self);
    }
}

fn terminate(helper: &mut HelperChild) {
    if helper.reaped {
        return;
    }
    // Kill the group even when its leader has exited: descendants may still
    // own our pipes. WNOWAIT keeps that leader PID reserved until this wait.
    if helper.id() > 1 {
        if let Some(pid) = rustix::process::Pid::from_raw(helper.id() as i32) {
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
        }
    }
    let _ = helper.child.kill();
    let _ = helper.child.wait();
    helper.reaped = true;
}

fn run(
    path: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    limit: usize,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, ErrorCode> {
    if cancelled() {
        return Err(ErrorCode::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(ErrorCode::InjectionFailed);
    }
    let mut child = command(path, args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .spawn()
        .map(HelperChild::new)
        .map_err(|_| ErrorCode::InjectionFailed)?;
    let stdout = child
        .child
        .stdout
        .take()
        .ok_or(ErrorCode::InjectionFailed)?;
    let reader = thread::spawn(move || {
        let mut output = Vec::new();
        stdout
            .take((limit + 1) as u64)
            .read_to_end(&mut output)
            .map(|_| output)
    });
    let writer = input.map(|bytes| {
        let mut bytes = bytes.to_vec();
        let mut stdin = child.child.stdin.take().expect("piped helper stdin");
        thread::spawn(move || {
            let result = stdin.write_all(&bytes);
            bytes.zeroize();
            result
        })
    });
    let result = loop {
        match child.exit_status() {
            Ok(Some(success)) => {
                break if success {
                    Ok(())
                } else {
                    Err(ErrorCode::InjectionFailed)
                }
            }
            Err(_) => break Err(ErrorCode::InjectionFailed),
            Ok(None) => {}
        }
        if cancelled() {
            break Err(ErrorCode::Cancelled);
        }
        if Instant::now() >= deadline {
            break Err(ErrorCode::InjectionFailed);
        }
        thread::sleep(Duration::from_millis(2));
    };
    terminate(&mut child);
    let written = writer.is_none_or(|writer| writer.join().is_ok_and(|result| result.is_ok()));
    let output = reader
        .join()
        .ok()
        .and_then(Result::ok)
        .ok_or(ErrorCode::InjectionFailed)?;
    result?;
    if !written || output.len() > limit {
        return Err(ErrorCode::InjectionFailed);
    }
    Ok(output)
}

impl X11Output {
    pub fn new(project_root: PathBuf) -> Self {
        let xdotool = executable(Path::new("/usr/bin/xdotool"))
            .or_else(|| executable(&project_root.join(".tools/xdotool/usr/bin/xdotool")));
        Self {
            xdotool,
            xclip: executable(Path::new("/usr/bin/xclip")),
            x11: std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("x11")
                && std::env::var_os("DISPLAY").is_some(),
            clipboard_owner: None,
        }
    }

    fn target(
        &self,
        deadline: Instant,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FocusTarget, ErrorCode> {
        if !self.x11 {
            return Err(ErrorCode::FocusChanged);
        }
        let tool = self.xdotool.as_deref().ok_or(ErrorCode::FocusChanged)?;
        focus::capture(|args| {
            run(
                tool,
                args,
                None,
                4096,
                deadline.min(Instant::now() + TOOL_TIMEOUT),
                cancelled,
            )
            .map_err(|code| {
                if code == ErrorCode::Cancelled {
                    code
                } else {
                    ErrorCode::FocusChanged
                }
            })
        })
    }
}

impl DesktopOutput for X11Output {
    fn capture_target(&mut self) -> Result<FocusTarget, ErrorCode> {
        self.target(Instant::now() + TOOL_TIMEOUT, &|| false)
    }

    fn insert(
        &mut self,
        target: &FocusTarget,
        text: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), ErrorCode> {
        if cancelled() {
            return Err(ErrorCode::Cancelled);
        }
        if text.is_empty()
            || text.len() > MAX_TEXT_BYTES
            || text
                .chars()
                .any(|value| value.is_control() || matches!(value, '\u{2028}' | '\u{2029}'))
        {
            return Err(ErrorCode::InjectionFailed);
        }
        let tool = self.xdotool.clone().ok_or(ErrorCode::InjectionFailed)?;
        let deadline = Instant::now() + INSERT_TIMEOUT;
        let mut characters = text.chars().peekable();
        while characters.peek().is_some() {
            if cancelled() {
                return Err(ErrorCode::Cancelled);
            }
            if self.target(deadline, cancelled)? != *target {
                return Err(ErrorCode::FocusChanged);
            }
            if cancelled() {
                return Err(ErrorCode::Cancelled);
            }
            let mut chunk: String = characters.by_ref().take(CHUNK_CHARACTERS).collect();
            // Let this small zero-delay block finish restoring modifiers. Cancellation
            // is checked immediately afterwards; no subsequent block is launched.
            let result = run(
                &tool,
                &["type", "--clearmodifiers", "--delay", "0", "--file", "-"],
                Some(chunk.as_bytes()),
                4096,
                deadline.min(Instant::now() + TOOL_TIMEOUT),
                &|| false,
            );
            chunk.zeroize();
            if cancelled() {
                return Err(ErrorCode::Cancelled);
            }
            result?;
        }
        Ok(())
    }

    fn copy(&mut self, text: &str) -> Result<(), ErrorCode> {
        if !self.x11 || text.is_empty() || text.len() > MAX_TEXT_BYTES {
            return Err(ErrorCode::ClipboardFailed);
        }
        let tool = self.xclip.as_deref().ok_or(ErrorCode::ClipboardFailed)?;
        let mut child = command(tool, &["-selection", "clipboard", "-in", "-quiet"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .map(HelperChild::new)
            .map_err(|_| ErrorCode::ClipboardFailed)?;
        let mut stdin = child.child.stdin.take().ok_or(ErrorCode::ClipboardFailed)?;
        let mut bytes = text.as_bytes().to_vec();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let writer = thread::spawn(move || {
            let result = stdin.write_all(&bytes);
            bytes.zeroize();
            drop(stdin);
            let _ = tx.send(result.is_ok());
        });
        let written = rx.recv_timeout(TOOL_TIMEOUT).unwrap_or(false);
        if !written {
            terminate(&mut child);
            let _ = writer.join();
            return Err(ErrorCode::ClipboardFailed);
        }
        let _ = writer.join();
        // Read-back acknowledges that the foreground owner has acquired selection.
        // No clipboard contents are logged or retained outside this comparison.
        let deadline = Instant::now() + TOOL_TIMEOUT;
        let acquired = loop {
            if !matches!(child.exit_status(), Ok(None)) || Instant::now() >= deadline {
                break false;
            }
            match run(
                tool,
                &["-selection", "clipboard", "-out", "-target", "UTF8_STRING"],
                None,
                MAX_TEXT_BYTES,
                deadline,
                &|| false,
            ) {
                Ok(mut contents) => {
                    let matches = contents == text.as_bytes();
                    contents.zeroize();
                    if matches {
                        break true;
                    }
                }
                Err(_) => break false,
            }
            thread::sleep(Duration::from_millis(5));
        };
        if !acquired {
            terminate(&mut child);
            return Err(ErrorCode::ClipboardFailed);
        }
        if let Some(mut previous) = self.clipboard_owner.replace(child) {
            terminate(&mut previous);
        }
        Ok(())
    }

    fn clear_clipboard(&mut self) -> Result<(), ErrorCode> {
        if let Some(mut child) = self.clipboard_owner.take() {
            // Killing our selection owner cannot overwrite a newer external owner.
            terminate(&mut child);
        }
        Ok(())
    }

    fn diagnostics(&self) -> OutputDiagnostics {
        OutputDiagnostics {
            x11: self.x11,
            xdotool_available: self.xdotool.is_some(),
            xclip_available: self.xclip.is_some(),
        }
    }
}

impl Drop for X11Output {
    fn drop(&mut self) {
        let _ = self.clear_clipboard();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn output() -> (tempfile::TempDir, X11Output) {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("active"), "42").unwrap();
        fs::write(root.path().join("focused"), "43").unwrap();
        let tool = root.path().join("xdotool");
        fs::write(
            &tool,
            r#"#!/bin/sh
cd "$(dirname "$0")" || exit 1
printf '%s\n' "$@" >> arguments
case "$1" in
getactivewindow) cat active ;;
getwindowfocus) cat focused ;;
getwindowgeometry) test ! -f destroyed ;;
getwindowpid) printf '2\n' ;;
type) cat >> typed; test ! -f change || printf '99' > active ;;
*) exit 1 ;;
esac
"#,
        )
        .unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
        let output = X11Output {
            xdotool: Some(tool),
            xclip: None,
            x11: true,
            clipboard_owner: None,
        };
        (root, output)
    }

    #[test]
    fn printable_unicode_is_sent_only_on_stdin() {
        let (root, mut output) = output();
        let target = output.capture_target().unwrap();
        output
            .insert(&target, "Bonjour émoji 🐢", &|| false)
            .unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join("typed")).unwrap(),
            "Bonjour émoji 🐢"
        );
        assert!(!fs::read_to_string(root.path().join("arguments"))
            .unwrap()
            .contains("Bonjour"));
    }

    #[test]
    fn changed_focus_and_destroyed_targets_never_type() {
        let (root, mut output) = output();
        let target = output.capture_target().unwrap();
        fs::write(root.path().join("active"), "99").unwrap();
        assert_eq!(
            output.insert(&target, "secret", &|| false),
            Err(ErrorCode::FocusChanged)
        );
        fs::write(root.path().join("active"), "42").unwrap();
        fs::write(root.path().join("destroyed"), "").unwrap();
        assert_eq!(
            output.insert(&target, "secret", &|| false),
            Err(ErrorCode::FocusChanged)
        );
        assert!(!root.path().join("typed").exists());
    }

    #[test]
    fn controls_and_pre_cancelled_work_never_type() {
        let (root, mut output) = output();
        let target = output.capture_target().unwrap();
        for text in ["bonjour\n", "bonjour\t", "bonjour\r", "bonjour\u{2028}"] {
            assert_eq!(
                output.insert(&target, text, &|| false),
                Err(ErrorCode::InjectionFailed)
            );
        }
        assert_eq!(
            output.insert(&target, "secret", &|| true),
            Err(ErrorCode::Cancelled)
        );
        assert!(!root.path().join("typed").exists());
    }

    #[test]
    fn changed_focus_between_blocks_stops_without_retry() {
        let (root, mut output) = output();
        let target = output.capture_target().unwrap();
        fs::write(root.path().join("change"), "").unwrap();
        assert_eq!(
            output.insert(&target, "1234567890123456789012345678901234567890", &|| {
                false
            }),
            Err(ErrorCode::FocusChanged)
        );
        assert!(fs::read(root.path().join("typed")).unwrap().len() <= CHUNK_CHARACTERS);
    }

    #[test]
    fn cancellation_between_blocks_prevents_further_typing() {
        let (root, mut output) = output();
        let target = output.capture_target().unwrap();
        assert_eq!(
            output.insert(&target, "1234567890123456789012345678901234567890", &|| {
                root.path().join("typed").exists()
            }),
            Err(ErrorCode::Cancelled)
        );
        assert!(fs::read(root.path().join("typed")).unwrap().len() <= CHUNK_CHARACTERS);
    }

    fn clipboard_tool(root: &Path) -> PathBuf {
        let tool = root.join("xclip");
        fs::write(
            &tool,
            r#"#!/bin/sh
cd "$(dirname "$0")" || exit 1
case " $* " in
*" -out "*) cat clipboard ;;
*) cat > clipboard; exec sleep 30 ;;
esac
"#,
        )
        .unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
        tool
    }

    #[test]
    fn clipboard_owner_is_reaped_and_external_contents_are_not_replaced() {
        let (root, mut output) = output();
        output.xclip = Some(clipboard_tool(root.path()));
        output.copy("texte privé").unwrap();
        let pid = output.clipboard_owner.as_ref().unwrap().id();
        assert_eq!(
            fs::read_to_string(root.path().join("clipboard")).unwrap(),
            "texte privé"
        );
        fs::write(root.path().join("clipboard"), "copie externe").unwrap();
        output.clear_clipboard().unwrap();
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        assert_eq!(
            fs::read_to_string(root.path().join("clipboard")).unwrap(),
            "copie externe"
        );
        output.clear_clipboard().unwrap();
    }

    #[test]
    fn dropping_output_reaps_its_foreground_clipboard_owner() {
        let (root, mut output) = output();
        output.xclip = Some(clipboard_tool(root.path()));
        output.copy("texte privé").unwrap();
        let pid = output.clipboard_owner.as_ref().unwrap().id();
        drop(output);
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
    }

    #[test]
    fn failed_clipboard_helper_is_bounded_and_keeps_previous_owner() {
        let (root, mut output) = output();
        output.xclip = Some(clipboard_tool(root.path()));
        output.copy("copie précédente").unwrap();
        let previous = output.clipboard_owner.as_ref().unwrap().id();
        let blocked = root.path().join("blocked-xclip");
        fs::write(&blocked, "#!/bin/sh\nexec sleep 30\n").unwrap();
        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o700)).unwrap();
        output.xclip = Some(blocked);
        let start = Instant::now();
        assert_eq!(
            output.copy(&"x".repeat(MAX_TEXT_BYTES)),
            Err(ErrorCode::ClipboardFailed)
        );
        assert!(start.elapsed() < Duration::from_secs(2));
        assert_eq!(output.clipboard_owner.as_ref().unwrap().id(), previous);
        assert!(output
            .clipboard_owner
            .as_mut()
            .unwrap()
            .exit_status()
            .unwrap()
            .is_none());
        assert_eq!(
            fs::read_to_string(root.path().join("clipboard")).unwrap(),
            "copie précédente"
        );
    }

    #[test]
    fn helper_timeout_is_bounded_and_child_is_reaped() {
        let root = tempfile::tempdir().unwrap();
        let tool = root.path().join("blocked");
        let pid_file = root.path().join("pid");
        fs::write(
            &tool,
            format!(
                "#!/bin/sh\nprintf '%s' $$ > '{}'\nexec sleep 30\n",
                pid_file.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
        let started = Instant::now();
        assert_eq!(
            run(
                &tool,
                &[],
                None,
                100,
                started + Duration::from_millis(100),
                &|| false
            ),
            Err(ErrorCode::InjectionFailed)
        );
        assert!(started.elapsed() < Duration::from_secs(1));
        let pid = fs::read_to_string(pid_file).unwrap();
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
    }

    #[test]
    fn exited_helper_descendant_cannot_hold_output_pipe_open() {
        let root = tempfile::tempdir().unwrap();
        let tool = root.path().join("exited-with-descendant");
        fs::write(&tool, "#!/bin/sh\nsleep 5 &\nexit 0\n").unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
        let start = Instant::now();
        assert_eq!(
            run(
                &tool,
                &[],
                None,
                100,
                start + Duration::from_millis(100),
                &|| false
            ),
            Ok(Vec::new())
        );
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn cancellation_after_final_focus_query_never_starts_typing() {
        let (root, mut output) = output();
        let tool = output.xdotool.as_ref().unwrap();
        let source = fs::read_to_string(tool).unwrap().replace(
            "getwindowfocus) cat focused ;;",
            "getwindowfocus) count=$(cat focus-count 2>/dev/null || printf 0); count=$((count + 1)); printf '%s' \"$count\" > focus-count; cat focused; if [ \"$count\" = 4 ]; then printf '%s' $$ > final-focus-pid; fi ;;",
        );
        fs::write(tool, source).unwrap();
        let target = output.capture_target().unwrap();
        let cancelled = || {
            fs::read_to_string(root.path().join("final-focus-pid"))
                .is_ok_and(|pid| !Path::new(&format!("/proc/{pid}")).exists())
        };
        assert_eq!(
            output.insert(&target, "must not be inserted", &cancelled),
            Err(ErrorCode::Cancelled)
        );
        assert!(!root.path().join("typed").exists());
    }

    #[test]
    fn own_application_is_not_an_insertion_target() {
        let (_root, mut output) = output();
        let tool = output.xdotool.as_ref().unwrap();
        let source = fs::read_to_string(tool).unwrap().replace(
            "printf '2\\n'",
            &format!("printf '{}\\n'", std::process::id()),
        );
        fs::write(tool, source).unwrap();
        assert_eq!(output.capture_target(), Err(ErrorCode::FocusChanged));
    }
}
