"""Single-job state machine. Only the control thread publishes responses."""
from dataclasses import dataclass
from pathlib import Path
import threading

from engine.protocol import ProtocolError, validate_model_path
from engine.transcriber import EngineError


def envelope(request_id, kind, **fields):
    return dict(protocol_version=1, request_id=request_id, type=kind, **fields)


def error_response(request_id, code):
    messages = {
        'ENGINE_BUSY': 'Une transcription est déjà en cours.',
        'PROTOCOL_ERROR': 'Requête du moteur invalide.',
        'ENGINE_CRASHED': 'Échec du moteur local.',
        'NO_SPEECH': 'Aucune parole détectée.',
        'MODEL_NOT_FOUND': 'Modèle local introuvable.',
        'MODEL_LOAD_FAILED': 'Modèle local impossible à charger.',
        'CANCELLED': 'Dictée annulée.',
        'ENGINE_TIMEOUT': 'Le service de transcription ne répond pas à temps.',
        'API_CONFIG_ERROR': 'Configuration Groq absente ou invalide.',
        'API_AUTH_ERROR': 'Clé API Groq refusée.',
        'API_RATE_LIMIT': 'Quota ou limite Groq atteint.',
        'API_UNAVAILABLE': 'Service Groq indisponible ou réponse invalide.',
    }
    return envelope(request_id, 'error', code=code, message=messages.get(code, 'Échec du moteur local.'))


@dataclass
class Job:
    request_id: str
    cancelled: threading.Event
    thread: threading.Thread | None = None
    result: object = None
    error: str | None = None


class EngineRuntime:
    def __init__(self, transcriber, emit):
        self.transcriber = transcriber
        self.emit = emit
        self.job = None
        self.closed = False

    @property
    def busy(self):
        return self.job is not None

    def handle(self, request):
        if self.closed:
            return
        request_id = request['request_id']
        if request['type'] == 'cancel':
            if self.job and self.job.request_id == request_id:
                self.job.cancelled.set()
            else:
                self.emit(error_response(request_id, 'PROTOCOL_ERROR'))
            return
        if self.job:
            # Replayed active identity must not create two terminal outcomes.
            if self.job.request_id != request_id:
                self.emit(error_response(request_id, 'ENGINE_BUSY'))
            return
        try:
            validate_model_path(request, str(self.transcriber.model_path))
        except ProtocolError:
            self.emit(error_response(request_id, 'PROTOCOL_ERROR'))
            return
        job = Job(request_id, threading.Event())
        self.job = job

        def work():
            try:
                job.result = self.transcriber.transcribe(Path(request['audio_path']), request['language'], job.cancelled)
            except EngineError as error:
                job.error = error.code
            except Exception:
                job.error = 'ENGINE_CRASHED'
        job.thread = threading.Thread(target=work, name='local-transcription', daemon=True)
        job.thread.start()

    def poll(self):
        job = self.job
        if job is None or job.thread.is_alive():
            return
        job.thread.join()
        self.job = None
        if self.closed:
            return
        if job.cancelled.is_set():
            self.emit(envelope(job.request_id, 'cancelled'))
        elif job.error:
            self.emit(error_response(job.request_id, job.error))
        else:
            result = job.result
            self.emit(envelope(job.request_id, 'result', text=result.text,
                               language=result.language, duration_ms=result.duration_ms))

    def shutdown(self):
        self.closed = True
        if self.job:
            self.job.cancelled.set()
