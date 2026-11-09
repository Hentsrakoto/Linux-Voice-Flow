"""Load only Groq configuration, without shell execution or interpolation."""
from dataclasses import dataclass, field
import os
from pathlib import Path
import re
import stat

from engine.errors import EngineError

ENV_PATH = Path(__file__).resolve().parents[1] / '.env'
DEFAULT_MODEL = 'whisper-large-v3'
MODELS = frozenset((DEFAULT_MODEL, 'whisper-large-v3-turbo'))


@dataclass(frozen=True)
class GroqConfig:
    api_key: str = field(repr=False)
    model: str = DEFAULT_MODEL


def load_groq_config(path=ENV_PATH, environment=None):
    environment = os.environ if environment is None else environment
    values = {}
    try:
        # A final symlink is never followed, and dotenv input is bounded.
        try:
            fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        except FileNotFoundError:
            fd = None
        if fd is not None:
            with os.fdopen(fd, 'rb') as source:
                if not stat.S_ISREG(os.fstat(source.fileno()).st_mode):
                    raise ValueError('config must be regular')
                raw = source.read(65537)
            if len(raw) > 65536:
                raise ValueError('config too large')
            for line in raw.decode('utf-8').splitlines():
                line = line.strip()
                if not line or line.startswith('#'):
                    continue
                if line.startswith('export '):
                    line = line[7:].lstrip()
                name, separator, value = line.partition('=')
                name = name.strip()
                if name not in ('GROQ_API_KEY', 'GROQ_MODEL'):
                    continue
                if not separator or name in values:
                    raise ValueError('invalid config assignment')
                value = value.strip()
                if value.startswith(('"', "'")):
                    if len(value) < 2 or value[-1] != value[0]:
                        raise ValueError('invalid quoted value')
                    value = value[1:-1]
                else:
                    value = value.split('#', 1)[0].strip()
                values[name] = value
        key = environment.get('GROQ_API_KEY', values.get('GROQ_API_KEY', ''))
        model = environment.get('GROQ_MODEL', values.get('GROQ_MODEL', DEFAULT_MODEL))
        if not re.fullmatch(r'[A-Za-z0-9_-]{1,4096}', key) or model not in MODELS:
            raise ValueError('invalid Groq config')
        return GroqConfig(key, model)
    except (OSError, ValueError, UnicodeError):
        raise EngineError('API_CONFIG_ERROR') from None
