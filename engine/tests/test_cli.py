import pytest
import json
import os
from pathlib import Path
import selectors
import subprocess
import sys
import textwrap


def read_line(process, timeout=3):
    with selectors.DefaultSelector() as selector:
        selector.register(process.stdout, selectors.EVENT_READ)
        assert selector.select(timeout), 'engine response timeout'
    return json.loads(process.stdout.readline())


def test_startup_failure_is_sanitized_and_never_ready(tmp_path):
    root = tmp_path / 'audio'
    root.mkdir(mode=0o700)
    process = subprocess.run([sys.executable, '-m', 'engine.main', '--model-path', str(tmp_path / 'absent'),
                              '--audio-root', str(root)], capture_output=True, timeout=5)
    messages = [json.loads(line) for line in process.stdout.splitlines()]
    assert process.returncode != 0
    assert len(messages) == 1
    assert messages[0]['code'] == 'MODEL_NOT_FOUND'
    assert str(tmp_path).encode() not in process.stdout + process.stderr


def test_real_control_loop_processes_cancel_during_blocking_work(tmp_path):
    # Dependency injection remains Python-only; the production CLI exposes no fake mode.
    script = textwrap.dedent('''
        import sys, threading
        from pathlib import Path
        from engine.main import run
        from engine.transcriber import TranscriptResult
        class Fake:
            model_path = Path('/local/model')
            def transcribe(self, audio, language, cancelled):
                cancelled.wait(5)
                return TranscriptResult('private-sentinel', 'fr', 1000)
        raise SystemExit(run(Fake(), sys.stdin.buffer, sys.stdout.buffer))
    ''')
    process = subprocess.Popen([sys.executable, '-c', script], stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        assert read_line(process)['type'] == 'ready'
        message = {'protocol_version': 1, 'request_id': 'd-1', 'type': 'transcribe',
                   'model_path': '/local/model', 'audio_path': '/private/audio.wav', 'language': 'fr', 'vad': True}
        line = json.dumps(message).encode() + b'\n'
        # Frame is deliberately fragmented across writes.
        process.stdin.write(line[:20])
        process.stdin.flush()
        process.stdin.write(line[20:])
        process.stdin.write(b'{"protocol_version":1,"request_id":"d-1","type":"cancel"}\n')
        process.stdin.flush()
        assert read_line(process)['type'] == 'cancelled'
        process.stdin.close()
        assert process.wait(3) == 0
        assert b'private-sentinel' not in process.stderr.read()
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()


def test_native_stdout_stderr_are_discarded(tmp_path):
    model = tmp_path / 'model'
    model.mkdir()
    for name in ('model.bin', 'config.json', 'tokenizer.json'):
        (model / name).write_text('{}')
    root = tmp_path / 'audio'
    root.mkdir(mode=0o700)
    script = textwrap.dedent('''
        import os, sys, types
        import engine.main
        class Model:
            def __init__(self, *a, **k):
                os.write(1, b'sensitive-native-path\\n')
                os.write(2, b'sensitive-native-text\\n')
        sys.modules['faster_whisper'] = types.SimpleNamespace(WhisperModel=Model)
        raise SystemExit(engine.main.main(sys.argv[1:]))
    ''')
    process = subprocess.run([sys.executable, '-c', script, '--model-path', str(model), '--audio-root', str(root)],
                             input=b'', capture_output=True, timeout=3)
    assert process.returncode == 0
    assert [json.loads(line)['type'] for line in process.stdout.splitlines()] == ['ready']
    assert process.stderr == b''


@pytest.mark.parametrize('payload', [b'{"type":', b'x' * (1024 * 1024 + 1)])
def test_truncated_or_oversized_frame_fails_closed(payload):
    script = "from engine.main import run; import sys; from pathlib import Path; fake=type('Fake',(),{'model_path':Path('/model')})(); raise SystemExit(run(fake,sys.stdin.buffer,sys.stdout.buffer))"
    process = subprocess.run([sys.executable, '-c', script], input=payload,
                             capture_output=True, timeout=3)
    messages = [json.loads(line) for line in process.stdout.splitlines()]
    assert [message['type'] for message in messages] == ['ready', 'error']
    assert messages[1]['code'] == 'PROTOCOL_ERROR'
    assert process.returncode == 1


def test_response_overflow_falls_back_to_bounded_protocol_error():
    base = {'protocol_version': 1, 'request_id': '', 'type': 'cancel'}
    overhead = len(json.dumps(base, separators=(',', ':')).encode())
    base['request_id'] = 'x' * (1024 * 1024 - overhead)
    payload = json.dumps(base, separators=(',', ':')).encode() + b'\n'
    script = "from engine.main import run; import sys; from pathlib import Path; fake=type('Fake',(),{'model_path':Path('/model')})(); raise SystemExit(run(fake,sys.stdin.buffer,sys.stdout.buffer))"
    process = subprocess.run([sys.executable, '-c', script], input=payload, capture_output=True, timeout=3)
    messages = [json.loads(line) for line in process.stdout.splitlines()]
    assert len(messages) == 2
    assert messages[1]['request_id'] == 'startup'
    assert messages[1]['code'] == 'PROTOCOL_ERROR'
    assert process.returncode == 0
    assert process.stderr == b''


def test_groq_startup_needs_no_local_weights_and_does_not_echo_key(tmp_path):
    root = tmp_path / 'audio'
    root.mkdir(mode=0o700)
    env = dict(os.environ, GROQ_API_KEY='test-secret-sentinel', GROQ_MODEL='whisper-large-v3')
    process = subprocess.run([sys.executable, '-m', 'engine.main', '--provider', 'groq',
                              '--model-path', str(tmp_path), '--audio-root', str(root)],
                             input=b'', capture_output=True, timeout=5, env=env)
    assert process.returncode == 0
    assert [json.loads(line)['type'] for line in process.stdout.splitlines()] == ['ready']
    assert b'test-secret-sentinel' not in process.stdout + process.stderr
    assert process.stderr == b''


def test_groq_startup_reports_sanitized_config_error(tmp_path):
    root = tmp_path / 'audio'
    root.mkdir(mode=0o700)
    env = dict(os.environ, GROQ_API_KEY='', GROQ_MODEL='whisper-large-v3')
    process = subprocess.run([sys.executable, '-m', 'engine.main', '--provider', 'groq',
                              '--model-path', str(tmp_path), '--audio-root', str(root)],
                             input=b'', capture_output=True, timeout=5, env=env)
    assert process.returncode == 1
    message, = [json.loads(line) for line in process.stdout.splitlines()]
    assert message['code'] == 'API_CONFIG_ERROR'
    assert str(tmp_path).encode() not in process.stdout + process.stderr
    assert process.stderr == b''


def test_groq_real_cli_jsonl_pipeline_with_mock_http(tmp_path):
    import wave
    root = tmp_path / 'audio'
    root.mkdir(mode=0o700)
    path = root / 'test.wav'
    with wave.open(str(path), 'wb') as wav:
        wav.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
        wav.writeframes(b'\0' * 32000)
    path.chmod(0o600)
    script = textwrap.dedent('''
        import io, sys
        import engine.main
        import engine.groq_transcriber as groq
        def transport(request, *, timeout):
            assert request.full_url == groq.ENDPOINT
            assert b'RIFF' in request.data
            return io.BytesIO(b'{"text":"Bonjour depuis Groq","language":"french"}')
        groq._transport = transport
        groq._has_speech = lambda pcm: True
        raise SystemExit(engine.main.main(sys.argv[1:]))
    ''')
    env = dict(os.environ, GROQ_API_KEY='test-secret-sentinel', GROQ_MODEL='whisper-large-v3')
    process = subprocess.Popen([sys.executable, '-c', script, '--provider', 'groq',
                                '--model-path', str(tmp_path), '--audio-root', str(root)],
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
    try:
        assert read_line(process)['type'] == 'ready'
        message = {'protocol_version': 1, 'request_id': 'dictation-1', 'type': 'transcribe',
                   'model_path': str(tmp_path), 'audio_path': str(path), 'language': 'fr', 'vad': True}
        process.stdin.write(json.dumps(message).encode() + b'\n')
        process.stdin.flush()
        result = read_line(process)
        assert result['type'] == 'result'
        assert result['text'] == 'Bonjour depuis Groq'
        assert result['duration_ms'] == 1000
        assert result['language'] == 'fr'
        process.stdin.close()
        assert process.wait(3) == 0
        assert process.stderr.read() == b''
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
