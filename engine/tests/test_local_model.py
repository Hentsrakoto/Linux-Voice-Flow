"""Opt-in integration. Never downloads a model or records a microphone."""
import os
import importlib
from pathlib import Path
import shutil
import socket
import wave

import pytest

from engine.transcriber import EngineError, Transcriber


@pytest.mark.local_model
def test_installed_faster_whisper_offline(tmp_path, monkeypatch):
    model = os.environ.get('LVF_TEST_MODEL_PATH')
    french = os.environ.get('LVF_TEST_FR_WAV')
    english = os.environ.get('LVF_TEST_EN_WAV')
    if not all((model, french, english)):
        pytest.skip('Supply LVF_TEST_MODEL_PATH, LVF_TEST_FR_WAV and LVF_TEST_EN_WAV explicitly')
    monkeypatch.setenv('ORT_DISABLE_TELEMETRY', '1')
    importlib.import_module('faster_whisper')
    def forbid_network(*args, **kwargs):
        raise AssertionError('Network forbidden during local integration')
    monkeypatch.setattr(socket.socket, 'connect', forbid_network)
    monkeypatch.setattr(socket, 'create_connection', forbid_network)
    root = tmp_path / 'private-audio'
    root.mkdir(mode=0o700)
    engine = Transcriber(Path(model), root)
    for source, language in ((french, 'fr'), (english, 'en')):
        target = root / f'{language}.wav'
        shutil.copyfile(source, target)
        target.chmod(0o600)
        result = engine.transcribe(target, language)
        assert result.text.strip()
        assert result.language == language
    silence = root / 'silence.wav'
    with wave.open(str(silence), 'wb') as stream:
        stream.setnchannels(1)
        stream.setsampwidth(2)
        stream.setframerate(16000)
        stream.writeframes(b'\0' * 32000)
    silence.chmod(0o600)
    with pytest.raises(EngineError) as error:
        engine.transcribe(silence, 'auto')
    assert error.value.code == 'NO_SPEECH'
