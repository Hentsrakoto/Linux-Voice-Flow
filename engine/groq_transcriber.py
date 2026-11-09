"""Explicit Groq provider. No upload occurs until transcribe is called."""
import json
import os
from pathlib import Path
import secrets
import socket
import threading
from urllib.error import HTTPError, URLError
from urllib.request import build_opener, HTTPRedirectHandler, Request

from engine.audio_input import absolute_directory, open_audio, validate_audio_root, validate_wav
from engine.errors import EngineError
from engine.groq_config import load_groq_config
from engine.transcriber import TranscriptResult

ENDPOINT = 'https://api.groq.com/openai/v1/audio/transcriptions'
TIMEOUT_SECONDS = 45
MAX_RESPONSE_BYTES = 1024 * 1024
LANGUAGES = {'fr': 'fr', 'french': 'fr', 'en': 'en', 'english': 'en'}


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def _transport(request, *, timeout):
    # Default HTTPS handler verifies certificates; never follow a redirect with a key.
    return build_opener(NoRedirect()).open(request, timeout=timeout)


def _has_speech(pcm):
    os.environ['ORT_DISABLE_TELEMETRY'] = '1'
    os.environ['HF_HUB_OFFLINE'] = '1'
    os.environ['HF_HUB_DISABLE_TELEMETRY'] = '1'
    import numpy as np
    from faster_whisper.vad import get_speech_timestamps
    samples = np.frombuffer(pcm, dtype='<i2').astype('float32') / 32768.0
    return bool(get_speech_timestamps(samples))


def _multipart(model, language, audio):
    boundary = 'voiceflow-' + secrets.token_hex(16)
    parts = []
    fields = {'model': model, 'response_format': 'verbose_json', 'temperature': '0'}
    if language != 'auto':
        fields['language'] = language
    for name, value in fields.items():
        parts.append(f'--{boundary}\r\nContent-Disposition: form-data; name="{name}"\r\n\r\n{value}\r\n'.encode())
    parts.extend((f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="dictation.wav"\r\nContent-Type: audio/wav\r\n\r\n'.encode(),
                  audio, f'\r\n--{boundary}--\r\n'.encode()))
    return b''.join(parts), f'multipart/form-data; boundary={boundary}'


class GroqTranscriber:
    def __init__(self, model_path, audio_root, *, config=None, transport=None, vad=None):
        self.config = config if config is not None else load_groq_config()
        try:
            self.model_path = absolute_directory(model_path)
            if not self.model_path.is_dir():
                raise ValueError('session identity must be directory')
        except (OSError, ValueError):
            raise EngineError('PROTOCOL_ERROR') from None
        self.audio_root = validate_audio_root(audio_root)
        self.transport = transport or _transport
        self.vad = vad or _has_speech

    def transcribe(self, audio_path: Path, language: str, cancelled: threading.Event | None = None):
        if language not in ('fr', 'en', 'auto'):
            raise EngineError('PROTOCOL_ERROR')
        cancelled = cancelled or threading.Event()
        try:
            if cancelled.is_set():
                raise EngineError('CANCELLED')
            with open_audio(audio_path, self.audio_root) as audio:
                duration_ms, pcm = validate_wav(audio)
                if not self.vad(pcm):
                    raise EngineError('NO_SPEECH')
                if cancelled.is_set():
                    raise EngineError('CANCELLED')
                body, content_type = _multipart(self.config.model, language, audio.read())
            request = Request(ENDPOINT, data=body, headers={
                'Authorization': 'Bearer ' + self.config.api_key,
                'Content-Type': content_type,
                'Accept': 'application/json',
                'User-Agent': 'LinuxVoiceFlow/0.1',
            }, method='POST')
            if cancelled.is_set():
                raise EngineError('CANCELLED')
            with self.transport(request, timeout=TIMEOUT_SECONDS) as response:
                raw = response.read(MAX_RESPONSE_BYTES + 1)
            if cancelled.is_set():
                raise EngineError('CANCELLED')
            if len(raw) > MAX_RESPONSE_BYTES:
                raise EngineError('API_UNAVAILABLE')
            result = json.loads(raw.decode('utf-8'))
            if not isinstance(result, dict) or not isinstance(result.get('text'), str):
                raise EngineError('API_UNAVAILABLE')
            text = result['text'].strip()
            if not text:
                raise EngineError('NO_SPEECH')
            detected = result.get('language', language if language != 'auto' else '')
            if not isinstance(detected, str):
                raise EngineError('API_UNAVAILABLE')
            detected = LANGUAGES.get(detected.lower())
            if detected is None:
                raise EngineError('NO_SPEECH')
            return TranscriptResult(text, detected, duration_ms)
        except EngineError:
            raise
        except HTTPError as error:
            code = 'API_AUTH_ERROR' if error.code == 401 else 'API_RATE_LIMIT' if error.code == 429 else 'API_UNAVAILABLE'
            error.close()
            raise EngineError(code) from None
        except (TimeoutError, socket.timeout):
            raise EngineError('ENGINE_TIMEOUT') from None
        except URLError as error:
            code = 'ENGINE_TIMEOUT' if isinstance(error.reason, (TimeoutError, socket.timeout)) else 'API_UNAVAILABLE'
            raise EngineError(code) from None
        except Exception:
            raise EngineError('API_UNAVAILABLE') from None
