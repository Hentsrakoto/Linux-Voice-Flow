import json, os, sys, time
from pathlib import Path
mode = Path('mode').read_text()
Path('pid').write_text(str(os.getpid()))
def emit(kind, rid='startup', **kw):
 print(json.dumps(dict(type=kind, protocol_version=1, request_id=rid, **kw)), flush=True)
if mode == 'startup_timeout': time.sleep(30)
if mode == 'startup_error':
 emit('error', code='MODEL_LOAD_FAILED', message='sanitized'); sys.exit(1)
if mode == 'provider_check':
 if '--provider' not in sys.argv or sys.argv[sys.argv.index('--provider')+1] != 'groq' or Path(sys.argv[sys.argv.index('--model-path')+1]) != Path.cwd():
  emit('error', code='MODEL_LOAD_FAILED', message='sanitized'); sys.exit(1)
emit('ready')
if mode == 'pipe_hang': time.sleep(30)
if mode == 'eof': sys.exit(0)
if mode == 'malformed': print('{}', flush=True); time.sleep(30)
if mode == 'oversize': print('x' * (1024*1024+1), flush=True); time.sleep(30)
for line in sys.stdin:
 req=json.loads(line); rid=req['request_id']
 if req['type']=='cancel':
  if mode != 'cancel_hang':
   emit('result', rid, text='late', language='fr', duration_ms=1)
   emit('cancelled',rid)
 elif mode == 'result':
  emit('result','unknown',text='ignored',language='fr',duration_ms=1)
  emit('result',rid,text='bonjour',language='fr',duration_ms=1)
