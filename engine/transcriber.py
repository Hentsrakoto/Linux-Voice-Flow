"""Offline-only adapter; Rust owns the temporary files and their removal."""
from dataclasses import dataclass
import logging
import os
from pathlib import Path
import threading

from engine.audio_input import absolute_directory, open_audio, validate_audio_root, validate_wav
from engine.errors import EngineError


@dataclass(frozen=True)
class TranscriptResult:
    text: str
    language: str
    duration_ms: int


class Transcriber:
    def __init__(self, model_path, audio_root, *, device='cpu', compute_type='int8', model_factory=None):
        try:
            self.model_path = absolute_directory(model_path)
            if not self.model_path.is_dir():
                raise ValueError('not directory')
        except FileNotFoundError:
            raise EngineError('MODEL_NOT_FOUND') from None
        except (ValueError, OSError):
            raise EngineError('MODEL_LOAD_FAILED') from None
        try:
            for name in ('model.bin', 'config.json', 'tokenizer.json'):
                asset = self.model_path / name
                if asset.is_symlink() or not asset.is_file() or asset.stat().st_size == 0:
                    raise ValueError('missing model asset')
        except (OSError, ValueError):
            raise EngineError('MODEL_LOAD_FAILED') from None
        self.audio_root = validate_audio_root(audio_root)
        try:
            if model_factory is None:
                # Defense in depth; complete local tokenizer is checked above.
                os.environ['HF_HUB_OFFLINE'] = '1'
                os.environ['HF_HUB_DISABLE_TELEMETRY'] = '1'
                # ONNX may initialize telemetry before its Python opt-out API runs.
                os.environ['ORT_DISABLE_TELEMETRY'] = '1'
                logging.getLogger('faster_whisper').disabled = True
                from faster_whisper import WhisperModel
                model_factory = WhisperModel
            self.model = model_factory(str(self.model_path), local_files_only=True,
                                       device=device, compute_type=compute_type)
        except Exception:
            raise EngineError('MODEL_LOAD_FAILED') from None

    def transcribe(self, audio_path: Path, language: str, cancelled: threading.Event | None = None) -> TranscriptResult:
        if language not in ('fr', 'en', 'auto'):
            raise EngineError('PROTOCOL_ERROR')
        cancelled = cancelled or threading.Event()
        with open_audio(audio_path, self.audio_root) as audio:
            duration_ms, _ = validate_wav(audio)
            try:
                if cancelled.is_set():
                    raise EngineError('CANCELLED')
                segments, info = self.model.transcribe(audio, language=None if language == 'auto' else language,
                                                       vad_filter=True, log_progress=False)
                texts = []
                for segment in segments:
                    if cancelled.is_set():
                        raise EngineError('CANCELLED')
                    texts.append(segment.text)
                if cancelled.is_set():
                    raise EngineError('CANCELLED')
                text = ''.join(texts).strip()
                if not text:
                    raise EngineError('NO_SPEECH')
                if info.language not in ('fr', 'en'):
                    raise EngineError('NO_SPEECH')
                return TranscriptResult(text, info.language, duration_ms)
            except EngineError:
                raise
            except Exception:
                raise EngineError('ENGINE_CRASHED') from None
