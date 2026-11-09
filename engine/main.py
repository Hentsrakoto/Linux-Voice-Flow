"""Linux JSONL CLI. Run with ``python -m engine.main`` (never a network service)."""
import argparse
import os
from pathlib import Path
import selectors
import sys

from engine.protocol import MAX_LINE_BYTES, ProtocolError, encode_message, parse_request
from engine.runtime import EngineRuntime, envelope, error_response
from engine.transcriber import EngineError, Transcriber


def run(transcriber, input_stream, output_stream):
    def emit(message):
        try:
            raw = encode_message(message)
        except ProtocolError:
            raw = encode_message(error_response(message['request_id'], 'ENGINE_CRASHED'))
        output_stream.write(raw)
        output_stream.flush()

    runtime = EngineRuntime(transcriber, emit)
    emit(envelope('startup', 'ready'))
    pending = bytearray()
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(input_stream, selectors.EVENT_READ)
            while True:
                events = selector.select(.02)
                # Commands already waiting on stdin take precedence over completed work.
                if events:
                    chunk = os.read(input_stream.fileno(), 65536)
                    if not chunk:
                        if pending:
                            emit(error_response('startup', 'PROTOCOL_ERROR'))
                            return 1
                        return 0
                    pending.extend(chunk)
                    while b'\n' in pending:
                        line, _, remaining = pending.partition(b'\n')
                        pending = bytearray(remaining)
                        if len(line) > MAX_LINE_BYTES:
                            emit(error_response('startup', 'PROTOCOL_ERROR'))
                            return 1
                        try:
                            runtime.handle(parse_request(bytes(line)))
                        except ProtocolError:
                            emit(error_response('startup', 'PROTOCOL_ERROR'))
                    if len(pending) > MAX_LINE_BYTES:
                        emit(error_response('startup', 'PROTOCOL_ERROR'))
                        return 1
                runtime.poll()
    except (BrokenPipeError, OSError):
        return 1
    finally:
        runtime.shutdown()


class SafeParser(argparse.ArgumentParser):
    def error(self, message):
        # argparse's default error can echo a sensitive command-line argument.
        self.exit(2, 'Arguments du moteur invalides.\n')


def main(argv=None):
    parser = SafeParser(description='Moteur de dictée local JSONL v1')
    parser.add_argument('--provider', choices=('local', 'groq'), default='local')
    parser.add_argument('--model-path', required=True, type=Path)
    parser.add_argument('--audio-root', required=True, type=Path)
    parser.add_argument('--device', choices=('cpu', 'cuda'), default='cpu')
    parser.add_argument('--compute-type', choices=('int8', 'float32', 'float16', 'int8_float16'), default='int8')
    args = parser.parse_args(argv)
    # Retain a dedicated protocol descriptor. Native libraries can print directly
    # to fd 1/2, bypassing Python logging; discard their unfiltered diagnostics.
    with os.fdopen(os.dup(sys.stdout.fileno()), 'wb') as protocol_output:
        with open(os.devnull, 'wb') as sink:
            os.dup2(sink.fileno(), sys.stdout.fileno())
            os.dup2(sink.fileno(), sys.stderr.fileno())
        try:
            if args.provider == 'groq':
                from engine.groq_transcriber import GroqTranscriber
                transcriber = GroqTranscriber(args.model_path, args.audio_root)
            else:
                transcriber = Transcriber(args.model_path, args.audio_root,
                                          device=args.device, compute_type=args.compute_type)
        except EngineError as error:
            protocol_output.write(encode_message(error_response('startup', error.code)))
            protocol_output.flush()
            return 1
        except Exception:
            protocol_output.write(encode_message(error_response('startup', 'MODEL_LOAD_FAILED')))
            protocol_output.flush()
            return 1
        return run(transcriber, sys.stdin.buffer, protocol_output)


if __name__ == '__main__':
    raise SystemExit(main())
