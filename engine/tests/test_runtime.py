import threading
import time
from pathlib import Path

from engine.runtime import EngineRuntime
from engine.transcriber import TranscriptResult


def request(id='d-1', type='transcribe'):
    message = {'protocol_version': 1, 'request_id': id, 'type': type}
    if type == 'transcribe':
        message.update(audio_path='/private/audio.wav', model_path='/local/model', language='fr', vad=True)
    return message


class BlockingTranscriber:
    model_path = Path('/local/model')
    def __init__(self):
        self.started = threading.Event()
        self.release = threading.Event()
    def transcribe(self, path, language, cancelled):
        self.started.set()
        self.release.wait(2)
        return TranscriptResult('sensitive text', 'fr', 1000)


def finish(runtime):
    deadline = time.monotonic() + 2
    while runtime.busy and time.monotonic() < deadline:
        runtime.poll()
        time.sleep(.001)
    assert not runtime.busy


def test_cancel_has_no_ack_until_worker_stopped_and_no_late_result():
    outputs = []
    transcriber = BlockingTranscriber()
    runtime = EngineRuntime(transcriber, outputs.append)
    runtime.handle(request())
    assert transcriber.started.wait(1)
    runtime.handle(request(type='cancel'))
    runtime.poll()
    assert outputs == []
    runtime.handle(request('d-2'))
    assert outputs[0]['code'] == 'ENGINE_BUSY'
    transcriber.release.set()
    finish(runtime)
    assert outputs[-1] == {'protocol_version': 1, 'request_id': 'd-1', 'type': 'cancelled'}
    assert not any(output['type'] == 'result' for output in outputs)


def test_full_result_emitted_once():
    outputs = []
    transcriber = BlockingTranscriber()
    transcriber.release.set()
    runtime = EngineRuntime(transcriber, outputs.append)
    runtime.handle(request())
    finish(runtime)
    runtime.poll()
    assert outputs == [{'protocol_version': 1, 'request_id': 'd-1', 'type': 'result',
                        'text': 'sensitive text', 'language': 'fr', 'duration_ms': 1000}]


def test_model_mismatch_never_starts_work():
    outputs = []
    transcriber = BlockingTranscriber()
    runtime = EngineRuntime(transcriber, outputs.append)
    message = request()
    message['model_path'] = '/other/model'
    runtime.handle(message)
    assert outputs[0]['code'] == 'PROTOCOL_ERROR'
    assert not transcriber.started.is_set()


def test_unknown_cancel_is_rejected():
    outputs = []
    runtime = EngineRuntime(BlockingTranscriber(), outputs.append)
    runtime.handle(request(type='cancel'))
    assert outputs[0]['code'] == 'PROTOCOL_ERROR'


def test_shutdown_neutralizes_running_result():
    outputs = []
    transcriber = BlockingTranscriber()
    runtime = EngineRuntime(transcriber, outputs.append)
    runtime.handle(request())
    assert transcriber.started.wait(1)
    runtime.shutdown()
    transcriber.release.set()
    finish(runtime)
    assert outputs == []
