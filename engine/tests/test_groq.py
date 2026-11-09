"""Groq adapter tests: no actual provider request or microphone recording."""
import io
import json
from pathlib import Path
import socket
import threading
from urllib.error import HTTPError, URLError
import wave

import pytest

from engine.groq_config import GroqConfig, load_groq_config
from engine.groq_transcriber import GroqTranscriber
from engine.transcriber import EngineError


@pytest.fixture
def private_audio(tmp_path):
    root = tmp_path / 'audio'
    root.mkdir(mode=0o700)
    path = root / 'private.wav'
    with wave.open(str(path), 'wb') as wav:
        wav.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
        wav.writeframes(b'\0' * 32000)
    path.chmod(0o600)
    return root, path


def config():
    return GroqConfig('secret-sentinel', 'whisper-large-v3')


class Response(io.BytesIO):
    pass


def adapter(private_audio, transport, vad=lambda pcm: True):
    root, _ = private_audio
    return GroqTranscriber(root.parent, root, config=config(), transport=transport, vad=vad)


def test_env_file_values_and_environment_precedence(tmp_path):
    env = tmp_path / '.env'
    env.write_text("# config\nGROQ_API_KEY='file-key'\nGROQ_MODEL=\"whisper-large-v3-turbo\"\nIGNORED=not-used\n")
    result = load_groq_config(env, {})
    assert result.api_key == 'file-key'
    assert result.model == 'whisper-large-v3-turbo'
    assert 'file-key' not in repr(result)
    assert load_groq_config(env, {'GROQ_API_KEY': 'env-key'}).api_key == 'env-key'


@pytest.mark.parametrize('content', ['', 'GROQ_API_KEY=', 'GROQ_API_KEY="unterminated',
                                    'GROQ_API_KEY=bad key', 'GROQ_API_KEY=key\nGROQ_MODEL=unknown'])
def test_invalid_config_is_sanitized(tmp_path, content):
    path = tmp_path / '.env'
    path.write_text(content)
    with pytest.raises(EngineError) as error:
        load_groq_config(path, {})
    assert error.value.code == 'API_CONFIG_ERROR'
    assert str(tmp_path) not in str(error.value)


def test_environment_only_config_and_default_model(tmp_path):
    result = load_groq_config(tmp_path / 'absent', {'GROQ_API_KEY': 'env-key'})
    assert result.model == 'whisper-large-v3'


@pytest.mark.parametrize('language,remote,expected', [('fr', 'french', 'fr'), ('en', 'english', 'en'), ('auto', 'french', 'fr')])
def test_upload_is_explicit_bounded_multipart(private_audio, language, remote, expected):
    calls = []
    def transport(request, *, timeout):
        calls.append(request)
        assert request.full_url == 'https://api.groq.com/openai/v1/audio/transcriptions'
        assert request.get_header('Authorization') == 'Bearer secret-sentinel'
        assert request.get_header('User-agent') == 'LinuxVoiceFlow/0.1'
        assert 0 < timeout < 120
        body = request.data
        assert b'name="model"\r\n\r\nwhisper-large-v3' in body
        assert b'name="response_format"\r\n\r\nverbose_json' in body
        assert b'filename="dictation.wav"' in body
        assert b'private.wav' not in body
        assert b'RIFF' in body
        assert (b'name="language"' in body) is (language != 'auto')
        return Response(json.dumps({'text': ' Bonjour ', 'language': remote}).encode())
    result = adapter(private_audio, transport).transcribe(private_audio[1], language)
    assert result.text == 'Bonjour'
    assert result.language == expected
    assert result.duration_ms == 1000
    assert len(calls) == 1


@pytest.mark.parametrize('status,code', [(401, 'API_AUTH_ERROR'), (403, 'API_UNAVAILABLE'), (429, 'API_RATE_LIMIT'), (400, 'API_UNAVAILABLE'), (500, 'API_UNAVAILABLE'), (302, 'API_UNAVAILABLE')])
def test_http_error_does_not_expose_remote_body(private_audio, status, code):
    def transport(*args, **kwargs):
        raise HTTPError('https://api.groq.com', status, 'secret-sentinel', {}, Response(b'private transcript'))
    with pytest.raises(EngineError) as error:
        adapter(private_audio, transport).transcribe(private_audio[1], 'fr')
    assert error.value.code == code
    assert 'secret-sentinel' not in str(error.value)
    assert 'private transcript' not in str(error.value)


@pytest.mark.parametrize('exception,code', [(socket.timeout('secret'), 'ENGINE_TIMEOUT'), (URLError('secret'), 'API_UNAVAILABLE')])
def test_network_error_is_sanitized(private_audio, exception, code):
    def transport(*args, **kwargs):
        raise exception
    with pytest.raises(EngineError) as error:
        adapter(private_audio, transport).transcribe(private_audio[1], 'fr')
    assert error.value.code == code


@pytest.mark.parametrize('payload,code', [(b'not-json', 'API_UNAVAILABLE'), (b'[]', 'API_UNAVAILABLE'),
                                        (b'{"text":8}', 'API_UNAVAILABLE'), (b'{"text":""}', 'NO_SPEECH'),
                                        (b'{"text":"hola","language":"spanish"}', 'NO_SPEECH'),
                                        (b'x' * (1024 * 1024 + 1), 'API_UNAVAILABLE')])
def test_response_is_checked(private_audio, payload, code):
    with pytest.raises(EngineError) as error:
        adapter(private_audio, lambda *a, **k: Response(payload)).transcribe(private_audio[1], 'auto')
    assert error.value.code == code


def test_silence_and_cancel_never_upload(private_audio):
    def transport(*args, **kwargs):
        pytest.fail('Unexpected network call')
    for cancelled, vad, code in [(threading.Event(), lambda pcm: False, 'NO_SPEECH'),
                                 (threading.Event(), lambda pcm: True, 'CANCELLED')]:
        if code == 'CANCELLED':
            cancelled.set()
        with pytest.raises(EngineError) as error:
            adapter(private_audio, transport, vad).transcribe(private_audio[1], 'fr', cancelled)
        assert error.value.code == code


def test_cancel_during_request_discards_late_text(private_audio):
    cancelled = threading.Event()
    def transport(*a, **k):
        cancelled.set()
        return Response(b'{"text":"late private text","language":"french"}')
    with pytest.raises(EngineError) as error:
        adapter(private_audio, transport).transcribe(private_audio[1], 'fr', cancelled)
    assert error.value.code == 'CANCELLED'


@pytest.mark.parametrize('unsafe', ['public', 'symlink', 'truncated', 'outside'])
def test_invalid_audio_never_uploads(private_audio, tmp_path, unsafe):
    root, path = private_audio
    if unsafe == 'public':
        path.chmod(0o644)
    elif unsafe == 'symlink':
        alias = root / 'alias.wav'
        alias.symlink_to(path)
        path = alias
    elif unsafe == 'truncated':
        path.write_bytes(path.read_bytes()[:-2])
    else:
        path = tmp_path / 'outside.wav'
    with pytest.raises(EngineError) as error:
        adapter(private_audio, lambda *a, **k: pytest.fail('network')).transcribe(path, 'fr')
    assert error.value.code == 'PROTOCOL_ERROR'


def test_real_vad_rejects_generated_silence_without_network(private_audio, monkeypatch):
    from engine.groq_transcriber import _has_speech
    def forbid(*a, **k):
        pytest.fail('No network permitted in local VAD')
    monkeypatch.setattr(socket.socket, 'connect', forbid)
    monkeypatch.setattr(socket, 'create_connection', forbid)
    transcriber = adapter(private_audio, forbid, _has_speech)
    with pytest.raises(EngineError) as error:
        transcriber.transcribe(private_audio[1], 'auto')
    assert error.value.code == 'NO_SPEECH'


def test_symlinked_or_oversized_config_is_rejected(tmp_path):
    source = tmp_path / 'source'
    source.write_text('GROQ_API_KEY=secret-sentinel')
    path = tmp_path / '.env'
    path.symlink_to(source)
    with pytest.raises(EngineError) as error:
        load_groq_config(path, {})
    assert error.value.code == 'API_CONFIG_ERROR'
    path.unlink()
    path.write_bytes(b'x' * 65537)
    with pytest.raises(EngineError) as error:
        load_groq_config(path, {})
    assert error.value.code == 'API_CONFIG_ERROR'


def test_api_redirects_are_not_followed():
    from engine.groq_transcriber import NoRedirect
    from urllib.request import Request
    request = Request('https://api.groq.com', headers={'Authorization': 'Bearer secret-sentinel'})
    assert NoRedirect().redirect_request(request, None, 302, 'found', {}, 'https://other.invalid') is None
