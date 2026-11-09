"""Bounded, private WAV input shared by local and remote providers."""
import os
from pathlib import Path
import stat
import wave

from engine.errors import EngineError


def absolute_directory(path):
    path = Path(path)
    if not path.is_absolute():
        raise ValueError('absolute directory required')
    # Reject symlinks anywhere in the supplied path, including ancestors.
    for component in (path, *path.parents):
        if component.is_symlink():
            raise ValueError('symlink directory')
    return path.resolve(strict=True)


def validate_audio_root(audio_root):
    try:
        root = absolute_directory(audio_root)
        metadata = root.stat()
        if not stat.S_ISDIR(metadata.st_mode) or stat.S_IMODE(metadata.st_mode) != 0o700 or metadata.st_uid != os.getuid():
            raise ValueError('audio root must be private')
        return root
    except (OSError, ValueError):
        raise EngineError('PROTOCOL_ERROR') from None


def open_audio(audio_path, audio_root):
    """Open beneath an owned 0700 root with openat/O_NOFOLLOW at every level."""
    path = Path(audio_path)
    if not path.is_absolute() or path.suffix.lower() != '.wav' or '..' in path.parts:
        raise EngineError('PROTOCOL_ERROR')
    try:
        relative = path.relative_to(audio_root)
        if not relative.parts:
            raise ValueError('not file')
        descriptor = os.open(audio_root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            for component in relative.parts[:-1]:
                next_fd = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=descriptor)
                os.close(descriptor)
                descriptor = next_fd
            audio_fd = os.open(relative.parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=descriptor)
        finally:
            os.close(descriptor)
        stream = os.fdopen(audio_fd, 'rb')
        try:
            metadata = os.fstat(stream.fileno())
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.getuid() or stat.S_IMODE(metadata.st_mode) != 0o600:
                raise ValueError('not private regular file')
            if metadata.st_size > 2_000_000:
                raise ValueError('audio too large')
            return stream
        except Exception:
            stream.close()
            raise
    except (OSError, ValueError):
        raise EngineError('PROTOCOL_ERROR') from None


def validate_wav(audio):
    try:
        with wave.open(audio, 'rb') as wav:
            frames = wav.getnframes()
            if wav.getnchannels() != 1 or wav.getframerate() != 16000 or wav.getsampwidth() != 2 or wav.getcomptype() != 'NONE':
                raise ValueError('format')
            if not 0 < frames <= 60 * 16000:
                raise ValueError('duration')
            pcm = wav.readframes(frames)
            if len(pcm) != frames * 2:
                raise ValueError('truncated')
        audio.seek(0)
        return round(frames * 1000 / 16000), pcm
    except (wave.Error, EOFError, OSError, ValueError):
        raise EngineError('PROTOCOL_ERROR') from None
