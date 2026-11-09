use crate::audio::{AudioBuffer, AudioError, MAX_SECONDS, OUTPUT_RATE};
use rustix::fs::{
    flock, fstat, mkdirat, openat, unlinkat, AtFlags, FlockOperation, Mode, OFlags, CWD,
};
use std::{
    fs::File,
    io::{BufWriter, Read, Write},
    os::fd::{AsRawFd, OwnedFd},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
const MARKER: &[u8] = b"linux-voice-flow-private-audio-v1\n";
static COUNTER: AtomicU64 = AtomicU64::new(0);
fn storage_error(_: impl std::fmt::Display) -> AudioError {
    AudioError::new("audio_storage", "Private audio storage unavailable")
}
fn dir_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
}
fn private(fd: &OwnedFd, directory: bool) -> Result<(), AudioError> {
    let stat = fstat(fd).map_err(storage_error)?;
    let expected = if directory { 0o700 } else { 0o600 };
    if stat.st_uid != rustix::process::geteuid().as_raw()
        || stat.st_mode & 0o777 != expected
        || (!directory && stat.st_nlink != 1)
    {
        return Err(storage_error("ownership/permissions"));
    }
    Ok(())
}
fn open_root(path: &Path) -> Result<OwnedFd, AudioError> {
    if !path.is_absolute() {
        return Err(storage_error("absolute path required"));
    }
    let components: Vec<_> = path.components().collect();
    if components.len() < 2
        || components
            .iter()
            .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
    {
        return Err(storage_error("invalid root"));
    }
    let mut fd = openat(CWD, "/", dir_flags(), Mode::empty()).map_err(storage_error)?;
    for (index, part) in components.iter().enumerate().skip(1) {
        let Component::Normal(name) = part else {
            return Err(storage_error("invalid path"));
        };
        if index == components.len() - 1 {
            match mkdirat(&fd, *name, Mode::from_raw_mode(0o700)) {
                Ok(()) => {}
                Err(rustix::io::Errno::EXIST) => {}
                Err(e) => return Err(storage_error(e)),
            }
        }
        fd = openat(&fd, *name, dir_flags(), Mode::empty()).map_err(storage_error)?;
    }
    private(&fd, true)?;
    Ok(fd)
}
struct Instance {
    root_fd: Arc<OwnedFd>,
    fd: OwnedFd,
    _lock: File,
    name: String,
    path: PathBuf,
}
impl Drop for Instance {
    fn drop(&mut self) {
        // Handles hold Arc<Instance>, so their WAVs have already been removed here.
        let _ = unlinkat(&self.fd, "owner.lock", AtFlags::empty());
        let _ = unlinkat(&*self.root_fd, self.name.as_str(), AtFlags::REMOVEDIR);
    }
}
pub struct TempAudioStore {
    root: PathBuf,
    root_fd: Arc<OwnedFd>,
    instance: Arc<Instance>,
}
pub struct TempAudioHandle {
    instance: Arc<Instance>,
    name: String,
    path: PathBuf,
    removed: bool,
}
impl TempAudioHandle {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn cleanup(&mut self) -> Result<(), AudioError> {
        if self.removed {
            return Ok(());
        }
        match unlinkat(&self.instance.fd, self.name.as_str(), AtFlags::empty()) {
            Ok(()) | Err(rustix::io::Errno::NOENT) => {
                self.removed = true;
                Ok(())
            }
            Err(e) => Err(storage_error(e)),
        }
    }
}
impl Drop for TempAudioHandle {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
impl TempAudioStore {
    pub fn new(base: &Path) -> Result<Self, AudioError> {
        let root_fd = Arc::new(open_root(base)?);
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(storage_error)?
            .as_nanos();
        let name = format!(
            "instance-{}-{nonce}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        mkdirat(&*root_fd, name.as_str(), Mode::from_raw_mode(0o700)).map_err(storage_error)?;
        let fd =
            openat(&*root_fd, name.as_str(), dir_flags(), Mode::empty()).map_err(storage_error)?;
        private(&fd, true)?;
        let lock_fd = openat(
            &fd,
            "owner.lock",
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(storage_error)?;
        private(&lock_fd, false)?;
        flock(&lock_fd, FlockOperation::NonBlockingLockExclusive).map_err(storage_error)?;
        let mut lock = File::from(lock_fd);
        lock.write_all(MARKER).map_err(storage_error)?;
        let instance = Arc::new(Instance {
            root_fd: root_fd.clone(),
            fd,
            _lock: lock,
            path: base.join(&name),
            name,
        });
        let store = Self {
            root: base.to_owned(),
            root_fd,
            instance,
        };
        store.cleanup_stale()?;
        Ok(store)
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn write_wav(
        &self,
        session_id: u64,
        buffer: &AudioBuffer,
    ) -> Result<TempAudioHandle, AudioError> {
        if buffer.sample_rate != OUTPUT_RATE
            || buffer.samples.is_empty()
            || buffer.samples.len() > OUTPUT_RATE as usize * MAX_SECONDS
        {
            return Err(AudioError::new(
                "audio_format",
                "Invalid normalized recording",
            ));
        }
        let name = format!("session-{session_id}.wav");
        let fd = openat(
            &self.instance.fd,
            name.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(storage_error)?;
        let handle = TempAudioHandle {
            instance: self.instance.clone(),
            path: self.instance.path.join(&name),
            name,
            removed: false,
        };
        private(&fd, false)?;
        let mut file = BufWriter::new(File::from(fd));
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: OUTPUT_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(&mut file, spec).map_err(storage_error)?;
        for sample in &buffer.samples {
            writer.write_sample(*sample).map_err(storage_error)?;
        }
        writer.finalize().map_err(storage_error)?;
        file.flush().map_err(storage_error)?;
        Ok(handle)
    }
    /// Only recognized files in locked, owned private instance directories are eligible.
    /// flock protects other live application instances; symlinks and foreign entries are preserved.
    pub fn cleanup_stale(&self) -> Result<(), AudioError> {
        let entries = std::fs::read_dir(format!("/proc/self/fd/{}", self.root_fd.as_raw_fd()))
            .map_err(storage_error)?;
        for entry in entries {
            let entry = entry.map_err(storage_error)?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if !valid_instance_name(name) || name == self.instance.name {
                continue;
            }
            let Ok(fd) = openat(&*self.root_fd, name, dir_flags(), Mode::empty()) else {
                continue;
            };
            if private(&fd, true).is_err() {
                continue;
            }
            let Ok(lock_fd) = openat(
                &fd,
                "owner.lock",
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
            ) else {
                continue;
            };
            if private(&lock_fd, false).is_err()
                || flock(&lock_fd, FlockOperation::NonBlockingLockExclusive).is_err()
            {
                continue;
            }
            let mut lock = File::from(lock_fd);
            let mut marker = vec![0; MARKER.len() + 1];
            let Ok(count) = lock.read(&mut marker) else {
                continue;
            };
            if &marker[..count] != MARKER {
                continue;
            }
            let Ok(contents) = std::fs::read_dir(format!("/proc/self/fd/{}", fd.as_raw_fd()))
            else {
                continue;
            };
            let mut files = Vec::new();
            let mut safe = true;
            for item in contents {
                let Ok(item) = item else {
                    safe = false;
                    break;
                };
                let filename = item.file_name();
                let Some(filename) = filename.to_str() else {
                    safe = false;
                    break;
                };
                if filename == "owner.lock" {
                    continue;
                }
                if !valid_wav_name(filename) {
                    safe = false;
                    break;
                }
                let Ok(audio) = openat(
                    &fd,
                    filename,
                    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                    Mode::empty(),
                ) else {
                    safe = false;
                    break;
                };
                let Ok(stat) = fstat(&audio) else {
                    safe = false;
                    break;
                };
                if private(&audio, false).is_err() || stat.st_mode & 0o170000 != 0o100000 {
                    safe = false;
                    break;
                }
                files.push(filename.to_owned());
            }
            if !safe {
                continue;
            }
            for file in files {
                unlinkat(&fd, file.as_str(), AtFlags::empty()).map_err(storage_error)?;
            }
            unlinkat(&fd, "owner.lock", AtFlags::empty()).map_err(storage_error)?;
            unlinkat(&*self.root_fd, name, AtFlags::REMOVEDIR).map_err(storage_error)?;
        }
        Ok(())
    }
}
fn valid_instance_name(name: &str) -> bool {
    let Some(suffix) = name.strip_prefix("instance-") else {
        return false;
    };
    let parts: Vec<_> = suffix.split('-').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
}
fn valid_wav_name(name: &str) -> bool {
    name.strip_prefix("session-")
        .and_then(|s| s.strip_suffix(".wav"))
        .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::{fs::symlink, fs::PermissionsExt};
    #[test]
    fn private_permissions_and_cleanup_on_all_exit_paths() {
        let parent = tempfile::tempdir().unwrap();
        let store = TempAudioStore::new(&parent.path().join("audio")).unwrap();
        assert_eq!(
            std::fs::metadata(store.root())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        let path;
        {
            let handle = store
                .write_wav(
                    1,
                    &AudioBuffer {
                        samples: vec![0; 160],
                        sample_rate: 16000,
                    },
                )
                .unwrap();
            path = handle.path().to_owned();
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            let wav = hound::WavReader::open(&path).unwrap();
            assert_eq!(wav.spec().sample_rate, 16000);
            assert_eq!(wav.spec().channels, 1);
            assert_eq!(wav.duration(), 160);
        }
        assert!(!path.exists());
    }
    #[test]
    fn stale_cleanup_does_not_follow_symlinks_or_delete_live_instance() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("audio");
        let store = TempAudioStore::new(&root).unwrap();
        let handle = store
            .write_wav(
                3,
                &AudioBuffer {
                    samples: vec![1],
                    sample_rate: 16000,
                },
            )
            .unwrap();
        let other = TempAudioStore::new(&root).unwrap();
        other.cleanup_stale().unwrap();
        assert!(handle.path().exists());
        let outside = parent.path().join("keep");
        std::fs::write(&outside, "foreign").unwrap();
        symlink(&outside, root.join("instance-123-456-789")).unwrap();
        other.cleanup_stale().unwrap();
        assert!(outside.exists());
        assert!(TempAudioStore::new(&root.join("instance-123-456-789").join("nested")).is_err());
    }
    #[test]
    fn stale_crash_recovery_preserves_foreign_files() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("audio");
        let store = TempAudioStore::new(&root).unwrap();
        let stale = root.join("instance-1-2-3");
        std::fs::create_dir(&stale).unwrap();
        std::fs::set_permissions(&stale, std::fs::Permissions::from_mode(0o700)).unwrap();
        for (name, bytes) in [
            ("owner.lock", MARKER),
            ("session-9.wav", &b"partial-crashed-wave"[..]),
        ] {
            std::fs::write(stale.join(name), bytes).unwrap();
            std::fs::set_permissions(stale.join(name), std::fs::Permissions::from_mode(0o600))
                .unwrap();
        }
        store.cleanup_stale().unwrap();
        assert!(!stale.exists());
        let foreign = root.join("instance-4-5-6");
        std::fs::create_dir(&foreign).unwrap();
        std::fs::set_permissions(&foreign, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(foreign.join("owner.lock"), MARKER).unwrap();
        std::fs::set_permissions(
            foreign.join("owner.lock"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        std::fs::write(foreign.join("keep.txt"), "foreign").unwrap();
        store.cleanup_stale().unwrap();
        assert!(foreign.join("keep.txt").exists());
    }
    #[test]
    fn invalid_audio_and_relative_or_symlink_roots_are_rejected() {
        assert!(TempAudioStore::new(Path::new("relative")).is_err());
        let parent = tempfile::tempdir().unwrap();
        let store = TempAudioStore::new(&parent.path().join("audio")).unwrap();
        assert!(store
            .write_wav(
                1,
                &AudioBuffer {
                    samples: vec![],
                    sample_rate: 16000
                }
            )
            .is_err());
        let link = parent.path().join("linked");
        symlink(store.root(), &link).unwrap();
        assert!(TempAudioStore::new(&link).is_err());
        let mut handle = store
            .write_wav(
                2,
                &AudioBuffer {
                    samples: vec![0],
                    sample_rate: 16000,
                },
            )
            .unwrap();
        assert!(store
            .write_wav(
                2,
                &AudioBuffer {
                    samples: vec![0],
                    sample_rate: 16000
                }
            )
            .is_err());
        handle.cleanup().unwrap();
        handle.cleanup().unwrap();
        assert!(!handle.path().exists());
    }
    #[test]
    fn handle_outlives_store_and_never_overwrites_symlink() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("audio");
        let store = TempAudioStore::new(&root).unwrap();
        let outside = parent.path().join("sentinel");
        std::fs::write(&outside, "keep").unwrap();
        let collision = store.instance.path.join("session-8.wav");
        symlink(&outside, &collision).unwrap();
        assert!(store
            .write_wav(
                8,
                &AudioBuffer {
                    samples: vec![1],
                    sample_rate: 16000
                }
            )
            .is_err());
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep");
        std::fs::remove_file(collision).unwrap();
        let handle = store
            .write_wav(
                9,
                &AudioBuffer {
                    samples: vec![1],
                    sample_rate: 16000,
                },
            )
            .unwrap();
        let path = handle.path().to_owned();
        drop(store);
        assert!(path.exists());
        drop(handle);
        assert!(!path.exists());
        assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
    }
    #[test]
    fn stale_cleanup_preserves_symlink_wav() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("audio");
        let store = TempAudioStore::new(&root).unwrap();
        let stale = root.join("instance-7-8-9");
        std::fs::create_dir(&stale).unwrap();
        std::fs::set_permissions(&stale, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(stale.join("owner.lock"), MARKER).unwrap();
        std::fs::set_permissions(
            stale.join("owner.lock"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let outside = parent.path().join("keep");
        std::fs::write(&outside, "keep").unwrap();
        symlink(&outside, stale.join("session-1.wav")).unwrap();
        store.cleanup_stale().unwrap();
        assert!(outside.exists());
        assert!(stale
            .join("session-1.wav")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());
    }
}
