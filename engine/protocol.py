"""Strict JSONL v1 boundary. Errors never expose input data."""
import json
from typing import BinaryIO

MAX_LINE_BYTES = 1024 * 1024
STARTUP_TIMEOUT_SECONDS = 60
TRANSCRIPTION_TIMEOUT_SECONDS = 120
CANCEL_TIMEOUT_SECONDS = 3
ERROR_CODES = frozenset(
    'MIC_UNAVAILABLE NO_SPEECH MODEL_NOT_FOUND MODEL_LOAD_FAILED ENGINE_BUSY '
    'ENGINE_TIMEOUT ENGINE_CRASHED FOCUS_CHANGED INJECTION_FAILED '
    'CLIPBOARD_FAILED CANCELLED PROTOCOL_ERROR '
    'API_CONFIG_ERROR API_AUTH_ERROR API_RATE_LIMIT API_UNAVAILABLE'.split()
)
FIELDS = {
    'transcribe': {'audio_path', 'language', 'model_path', 'vad'},
    'cancel': set(),
    'ready': set(),
    'result': {'text', 'language', 'duration_ms'},
    'error': {'code', 'message'},
    'cancelled': set(),
}
REQUEST_TYPES = ('transcribe', 'cancel')
RESPONSE_TYPES = ('ready', 'result', 'error', 'cancelled')


class ProtocolError(ValueError):
    code = 'PROTOCOL_ERROR'

    def __init__(self):
        super().__init__('Message du moteur invalide')


def _pairs(pairs):
    """Keep duplicate keys from being silently overwritten by json.loads."""
    result = {}
    for key, value in pairs:
        if key in result:
            raise ProtocolError()
        result[key] = value
    return result


def _invalid_constant(_):
    raise ProtocolError()


def _validate(obj, allowed):
    if type(obj) is not dict:
        raise ProtocolError()
    kind = obj.get('type')
    if type(kind) is not str or kind not in allowed:
        raise ProtocolError()
    if set(obj) != FIELDS[kind] | {'type', 'protocol_version', 'request_id'}:
        raise ProtocolError()
    # bool is an int subclass in Python, but never a protocol integer.
    if type(obj['protocol_version']) is not int or obj['protocol_version'] != 1:
        raise ProtocolError()
    request_id = obj['request_id']
    if type(request_id) is not str or not request_id:
        raise ProtocolError()
    if kind == 'ready' and request_id != 'startup':
        raise ProtocolError()
    if kind not in ('ready', 'error') and request_id == 'startup':
        raise ProtocolError()

    if kind == 'transcribe':
        for name in ('audio_path', 'model_path'):
            path = obj[name]
            if type(path) is not str or not path.startswith('/') or '\0' in path:
                raise ProtocolError()
        if obj['language'] not in ('fr', 'en', 'auto') or obj['vad'] is not True:
            raise ProtocolError()
    elif kind == 'result':
        if type(obj['text']) is not str or not obj['text']:
            raise ProtocolError()
        if obj['language'] not in ('fr', 'en'):
            raise ProtocolError()
        duration = obj['duration_ms']
        if type(duration) is not int or not 0 <= duration <= 9007199254740991:
            raise ProtocolError()
    elif kind == 'error':
        if type(obj['code']) is not str or obj['code'] not in ERROR_CODES:
            raise ProtocolError()
        if type(obj['message']) is not str or not obj['message']:
            raise ProtocolError()

    # JSON can escape lone surrogates; they are not interoperable UTF-8 strings.
    try:
        for value in obj.values():
            if isinstance(value, str):
                value.encode('utf-8', errors='strict')
    except UnicodeError:
        raise ProtocolError() from None
    return obj


def _parse(raw, allowed):
    if not isinstance(raw, bytes):
        raise ProtocolError()
    if raw.endswith(b'\n'):
        raw = raw[:-1]
    if len(raw) > MAX_LINE_BYTES or b'\n' in raw or b'\r' in raw:
        raise ProtocolError()
    try:
        obj = json.loads(
            raw.decode('utf-8', errors='strict'),
            object_pairs_hook=_pairs,
            parse_constant=_invalid_constant,
        )
    except (ValueError, UnicodeError, RecursionError):
        raise ProtocolError() from None
    return _validate(obj, allowed)


def parse_request(raw: bytes) -> dict:
    return _parse(raw, REQUEST_TYPES)


def parse_response(raw: bytes) -> dict:
    return _parse(raw, RESPONSE_TYPES)


def encode_message(obj: dict) -> bytes:
    _validate(obj, REQUEST_TYPES + RESPONSE_TYPES)
    raw = json.dumps(
        obj, ensure_ascii=False, separators=(',', ':'), allow_nan=False,
    ).encode('utf-8')
    if len(raw) > MAX_LINE_BYTES:
        raise ProtocolError()
    return raw + b'\n'


def read_message(stream: BinaryIO) -> dict | None:
    """Read one bounded line; only clean EOF returns None."""
    raw = stream.readline(MAX_LINE_BYTES + 2)
    if not raw:
        return None
    if not raw.endswith(b'\n'):
        raise ProtocolError()
    return _parse(raw, REQUEST_TYPES + RESPONSE_TYPES)


def validate_model_path(request: dict, loaded_path: str) -> None:
    if request.get('type') == 'transcribe' and request.get('model_path') != loaded_path:
        raise ProtocolError()
