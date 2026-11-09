"""Cross-language boundary tests: removing any strict validator breaks these cases."""
import io
import json
from pathlib import Path
import unittest
from engine import protocol as p

FIXTURES = Path(__file__).resolve().parents[2] / 'contracts/fixtures/messages.json'

class ProtocolTests(unittest.TestCase):
    def test_shared_messages(self):
        for case in json.loads(FIXTURES.read_text()):
            with self.subTest(case=case['name']):
                parser = p.parse_request if case['direction'] == 'request' else p.parse_response
                raw = case['json'].encode()
                if case['valid']:
                    self.assertEqual(parser(raw), json.loads(case['json']))
                    self.assertEqual(parser(p.encode_message(parser(raw))), parser(raw))
                else:
                    with self.assertRaises(p.ProtocolError): parser(raw)

    def test_utf8_nonfinite_duplicates_and_size(self):
        for raw in [b'\xff', b'{"type":NaN}', b'{"type":Infinity}', b'{"type":"ready","type":"ready","protocol_version":1,"request_id":"startup"}', b' ' * (p.MAX_LINE_BYTES + 1)]:
            with self.subTest(raw=raw[:20]):
                with self.assertRaises(p.ProtocolError): p.parse_response(raw)

    def test_exact_byte_boundary_and_stream(self):
        base = b'{"type":"error","protocol_version":1,"request_id":"startup","code":"PROTOCOL_ERROR","message":"X"}'
        exact = base.replace(b'X', b'x' * (p.MAX_LINE_BYTES - len(base) + 1))
        self.assertEqual(len(exact), p.MAX_LINE_BYTES)
        self.assertEqual(p.parse_response(exact)['code'], 'PROTOCOL_ERROR')
        with self.assertRaises(p.ProtocolError): p.parse_response(exact + b' ')
        ready = b'{"type":"ready","protocol_version":1,"request_id":"startup"}\n'
        stream = io.BytesIO(ready + ready)
        self.assertEqual(p.read_message(stream)['type'], 'ready')
        self.assertEqual(p.read_message(stream)['type'], 'ready')
        self.assertIsNone(p.read_message(stream))
        with self.assertRaises(p.ProtocolError): p.read_message(io.BytesIO(ready[:-1]))
        with self.assertRaises(p.ProtocolError): p.read_message(io.BytesIO(b'x' * (p.MAX_LINE_BYTES + 2)))

    def test_schema_agrees_with_shared_semantic_fixtures(self):
        try:
            from jsonschema import Draft202012Validator
        except ImportError:
            self.skipTest('Optional jsonschema test dependency unavailable')
        schema = json.loads((FIXTURES.parents[1] / 'engine-v1.schema.json').read_text())
        Draft202012Validator.check_schema(schema)
        validator = Draft202012Validator(schema)
        for case in json.loads(FIXTURES.read_text()):
            if case.get('wire_only') or case['name'] == 'wrong-direction': continue
            try: obj = json.loads(case['json'])
            except ValueError: continue
            with self.subTest(case=case['name']):
                self.assertEqual(validator.is_valid(obj),case['valid'])

    def test_loaded_model_identity(self):
        request = dict(protocol_version=1,request_id='d-1',type='transcribe',audio_path='/tmp/a.wav',language='auto',model_path='/models/a',vad=True)
        p.validate_model_path(request, '/models/a')
        with self.assertRaises(p.ProtocolError): p.validate_model_path(request, '/models/b')

if __name__ == '__main__': unittest.main()
