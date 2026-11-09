#!/usr/bin/env python3
"""Check the production transcriber offline with generated silence only."""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time
import wave

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))


def check(model_id):
    os.environ['HF_HUB_OFFLINE'] = '1'
    os.environ['HF_HUB_DISABLE_TELEMETRY'] = '1'
    os.environ['TRANSFORMERS_OFFLINE'] = '1'
    def blocked(*args, **kwargs):
        raise RuntimeError('Network disabled during model validation')
    socket.socket.connect = blocked
    socket.create_connection = blocked
    from engine.transcriber import EngineError, Transcriber
    start = time.monotonic()
    with tempfile.TemporaryDirectory(prefix='wispr-model-check-') as temporary:
        os.chmod(temporary, 0o700)
        model = Transcriber(ROOT / 'models' / model_id, Path(temporary))
        loaded = time.monotonic()
        path = Path(temporary) / 'silence.wav'
        with wave.open(str(path), 'wb') as audio:
            audio.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
            audio.writeframes(b'\0' * 32000)
        path.chmod(0o600)
        try:
            model.transcribe(path, language='fr')
        except EngineError as error:
            if error.code != 'NO_SPEECH':
                raise
        else:
            raise RuntimeError('Silence unexpectedly produced a transcript')
    print(json.dumps({'model': model_id, 'device': 'cpu', 'compute_type': 'int8', 'load_seconds': round(loaded-start, 3), 'total_seconds': round(time.monotonic()-start, 3), 'result': 'NO_SPEECH', 'network': 'blocked'}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model', choices=['tiny', 'base'])
    args = parser.parse_args()
    if args.model:
        check(args.model)
    else:
        installed = [identifier for identifier in ['tiny', 'base']
                     if all((ROOT / 'models' / identifier / asset).is_file()
                            for asset in ['model.bin', 'config.json', 'tokenizer.json', 'vocabulary.txt'])]
        if not installed:
            parser.error('Aucun modèle installé. Lancez scripts/install_models.py tiny auparavant.')
        for identifier in installed:
            subprocess.run([sys.executable, str(Path(__file__).resolve()), '--model', identifier], check=True)
