import threading
import wave
from pathlib import Path
from types import SimpleNamespace

import pytest

from engine.transcriber import EngineError, Transcriber


@pytest.fixture
def model_dir(tmp_path):
    root = tmp_path / 'model'
    root.mkdir()
    for name in ('model.bin', 'config.json', 'tokenizer.json'):
        (root / name).write_text('{}')
    return root


@pytest.fixture
def audio_root(tmp_path):
    root = tmp_path / 'audio'
    root.mkdir(mode=0o700)
    return root


def wav(root, name='silence.wav', seconds=1, rate=16000, channels=1):
    path = root / name
    with wave.open(str(path), 'wb') as stream:
        stream.setnchannels(channels)
        stream.setsampwidth(2)
        stream.setframerate(rate)
        stream.writeframes(b'\0' * rate * channels * 2 * seconds)
    path.chmod(0o600)
    return path


class Model:
    def __init__(self, segments=(), language='fr'):
        self.segments = segments
        self.language = language
        self.options = None

    def transcribe(self, audio, **options):
        self.options = options
        return iter(self.segments), SimpleNamespace(language=self.language)


def create(model_dir, audio_root, model):
    return Transcriber(model_dir, audio_root, model_factory=lambda *a, **k: model)


def test_missing_model_never_downloads(tmp_path, audio_root):
    called = []
    with pytest.raises(EngineError) as error:
        Transcriber(tmp_path / 'absent', audio_root, model_factory=lambda *a, **k: called.append(a))
    assert error.value.code == 'MODEL_NOT_FOUND'
    assert called == []


def test_missing_tokenizer_never_loads(model_dir, audio_root):
    (model_dir / 'tokenizer.json').unlink()
    with pytest.raises(EngineError) as error:
        create(model_dir, audio_root, Model())
    assert error.value.code == 'MODEL_LOAD_FAILED'


def test_corrupt_model_reports_load_error(model_dir, audio_root):
    def broken(*args, **kwargs):
        raise RuntimeError('private-path-and-text')
    with pytest.raises(EngineError) as error:
        Transcriber(model_dir, audio_root, model_factory=broken)
    assert error.value.code == 'MODEL_LOAD_FAILED'
    assert 'private' not in str(error.value)


def test_silence_returns_no_speech(model_dir, audio_root):
    with pytest.raises(EngineError) as error:
        create(model_dir, audio_root, Model()).transcribe(wav(audio_root), 'auto')
    assert error.value.code == 'NO_SPEECH'


@pytest.mark.parametrize('language,forwarded', [('fr', 'fr'), ('en', 'en'), ('auto', None)])
def test_language_is_forwarded(model_dir, audio_root, language, forwarded):
    model = Model([SimpleNamespace(text=' Bonjour '), SimpleNamespace(text='monde.')])
    result = create(model_dir, audio_root, model).transcribe(wav(audio_root), language)
    assert result.text == 'Bonjour monde.'
    assert result.duration_ms == 1000
    assert model.options['language'] == forwarded
    assert model.options['vad_filter'] is True


def test_local_only_factory(model_dir, audio_root):
    options = {}
    def factory(path, **kwargs):
        options.update(kwargs)
        assert path == str(model_dir)
        return Model()
    Transcriber(model_dir, audio_root, model_factory=factory)
    assert options['local_files_only'] is True
    assert options['device'] == 'cpu'
    assert options['compute_type'] == 'int8'


def test_telemetry_is_disabled_before_loading_native_runtime(model_dir, audio_root, monkeypatch):
    import os
    import sys

    monkeypatch.delenv('ORT_DISABLE_TELEMETRY', raising=False)

    def factory(*args, **kwargs):
        assert os.environ.get('ORT_DISABLE_TELEMETRY') == '1'
        return Model()

    monkeypatch.setitem(sys.modules, 'faster_whisper', SimpleNamespace(WhisperModel=factory))
    Transcriber(model_dir, audio_root)


@pytest.mark.parametrize('kind', ['outside', 'symlink', 'directory', 'badwav', 'stereo', 'rate', 'long'])
def test_rejects_untrusted_audio(model_dir, audio_root, tmp_path, kind):
    if kind == 'outside':
        path = wav(tmp_path)
    elif kind == 'symlink':
        target = wav(audio_root)
        path = audio_root / 'link.wav'
        path.symlink_to(target)
    elif kind == 'directory':
        path = audio_root / 'dir.wav'
        path.mkdir()
    elif kind == 'badwav':
        path = audio_root / 'bad.wav'
        path.write_bytes(b'not a wave')
    else:
        path = wav(audio_root, channels=2 if kind == 'stereo' else 1,
                   rate=8000 if kind == 'rate' else 16000,
                   seconds=61 if kind == 'long' else 1)
    with pytest.raises(EngineError) as error:
        create(model_dir, audio_root, Model()).transcribe(path, 'fr')
    assert error.value.code == 'PROTOCOL_ERROR'


def test_lazy_failure_is_sanitized(model_dir, audio_root):
    class Broken:
        def transcribe(self, *a, **k):
            def segments():
                yield SimpleNamespace(text='sensitive')
                raise RuntimeError('sensitive')
            return segments(), SimpleNamespace(language='fr')
    with pytest.raises(EngineError) as error:
        create(model_dir, audio_root, Broken()).transcribe(wav(audio_root), 'fr')
    assert error.value.code == 'ENGINE_CRASHED'
    assert 'sensitive' not in str(error.value)


def test_rejects_symlink_ancestor_audio(model_dir, audio_root):
    directory = audio_root / 'nested'
    directory.mkdir(mode=0o700)
    target = wav(directory)
    link = audio_root / 'alias'
    link.symlink_to(directory, target_is_directory=True)
    with pytest.raises(EngineError) as error:
        create(model_dir, audio_root, Model()).transcribe(link / target.name, 'fr')
    assert error.value.code == 'PROTOCOL_ERROR'


def test_rejects_public_audio_and_truncated_payload(model_dir, audio_root):
    path = wav(audio_root)
    path.chmod(0o644)
    transcriber = create(model_dir, audio_root, Model())
    with pytest.raises(EngineError) as error:
        transcriber.transcribe(path, 'fr')
    assert error.value.code == 'PROTOCOL_ERROR'
    path.chmod(0o600)
    path.write_bytes(path.read_bytes()[:-2])
    with pytest.raises(EngineError) as error:
        transcriber.transcribe(path, 'fr')
    assert error.value.code == 'PROTOCOL_ERROR'


def test_cancelled_before_native_call_never_transcribes(model_dir, audio_root):
    model = Model([SimpleNamespace(text='private')])
    transcriber = create(model_dir, audio_root, model)
    cancelled = threading.Event()
    cancelled.set()
    with pytest.raises(EngineError) as error:
        transcriber.transcribe(wav(audio_root), 'fr', cancelled)
    assert error.value.code == 'CANCELLED'
    assert model.options is None


def test_public_audio_root_reports_protocol_error(model_dir, audio_root):
    audio_root.chmod(0o777)
    with pytest.raises(EngineError) as error:
        create(model_dir, audio_root, Model())
    assert error.value.code == 'PROTOCOL_ERROR'
