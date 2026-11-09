"""Decode real PCM without model weights to catch incompatible runtime wheels."""
import socket
import wave

import pytest


def test_faster_whisper_decodes_pcm_with_installed_pyav(tmp_path, monkeypatch):
    monkeypatch.setenv('ORT_DISABLE_TELEMETRY', '1')
    audio = pytest.importorskip('faster_whisper.audio')

    def forbid_network(*args, **kwargs):
        raise AssertionError('Network forbidden during audio decoding')

    monkeypatch.setattr(socket.socket, 'connect', forbid_network)
    monkeypatch.setattr(socket, 'create_connection', forbid_network)
    path = tmp_path / 'silence.wav'
    with wave.open(str(path), 'wb') as stream:
        stream.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
        stream.writeframes(b'\0' * 3200)

    with path.open('rb') as stream:
        decoded = audio.decode_audio(stream, sampling_rate=16000)

    assert decoded.shape == (1600,)
    assert not decoded.any()
