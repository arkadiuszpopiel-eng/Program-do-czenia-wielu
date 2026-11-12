"""Porównanie starej reprezentacji i faktycznego odczytu danych kontekstu; bez akcji komputera."""
import datetime
import hashlib
import http.client
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import sys
import threading
import time

ROOT = Path('/workspace/scratch/repair-20261111')
REPO = Path('/workspace/wt-nightly')
SERVER = Path('/workspace/tools/llama/llama-b11559/llama-server')
ENTRY = json.loads((ROOT/'main-model-entry.json').read_text())
FIXTURE = REPO/'docs/reviews/artifacts/history-receipts-v1-candidate-responses.json'
VARIANT = sys.argv[1]
assert VARIANT == 'diag_minimal'
assert json.loads((ROOT/'main-model-download.json').read_text()).get('verification') == 'MATCH'
assert hashlib.sha256(SERVER.read_bytes()).hexdigest() == '16c4e5fc3e0a74400033d958722e487a797dbb51c676f5ce1ec9066f1673efd5'
OUT = ROOT/('raw-'+VARIANT)
OUT.mkdir(exist_ok=False)

def stamp():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()

def read_ledger(case):
    """Rzeczywisty odczyt syntetycznego fixture, nie deklaracja ani wykonanie akcji użytkownika."""
    notes = [m for m in case['provider_request']['messages'] if m['role'] == 'system']
    assert len(notes) == 1
    note = ''.join(b['text'] for b in notes[0]['content'])
    counts = json.loads(note.splitlines()[1])
    assert set(counts) == {'recorded_done', 'needs_confirmation', 'error_or_denied', 'recorded_running', 'undone', 'unverified'}
    assert all(isinstance(n, int) and n >= 0 for n in counts.values())
    return {'source':'application_execution_ledger',
            'scope':'selected_branch_before_current_turn',
            'lookup_is_not_agent_action':True, 'counts':counts,
            'limits':'Recorded states only. Model text is not execution evidence; Done is not proof of tests or effects outside the record.'}

def request_body(case):
    return {'model':ENTRY['id'],'messages':[{'role':'user','content':'Odpowiedz słowem GOTOWA.'}], 'max_tokens':8,'temperature':0,'stream':True,'stream_options':{'include_usage':True}}
    req = case['provider_request']
    messages = [{'role':'system', 'content':req['system']}]
    for m in req['messages']:
        if m['role'] == 'system':
            continue  # Odrzucona nota V1 nie jest kodem produktu ani częścią kandydata.
        assert all(b['type'] == 'text' for b in m['content'])
        messages.append({'role':m['role'], 'content':''.join(b['text'] for b in m['content'])})
    assert messages[-1]['role'] == 'user'
    if VARIANT == 'host_lookup':
        data = read_ledger(case)
        messages[-1:-1] = [
            {'role':'assistant', 'content':None, 'tool_calls':[{
                'id':'alfa-context-ledger', 'type':'function',
                'function':{'name':'alfa_execution_ledger', 'arguments':'{}'}}]},
            {'role':'tool', 'tool_call_id':'alfa-context-ledger',
             'content':json.dumps(data, ensure_ascii=False)}]
    return {'model':ENTRY['id'], 'messages':messages, 'max_tokens':96,
            'temperature':0, 'stream':True, 'stream_options':{'include_usage':True}}

with socket.socket() as sock:
    sock.bind(('127.0.0.1',0))
    port = sock.getsockname()[1]
auth = secrets.token_urlsafe(32)  # Nowy klucz tego serwera, bez odczytu kont/CLI.
args = [str(SERVER), '--host','127.0.0.1', '--port',str(port), '-m',
        str(Path('/workspace/models')/ENTRY['file']), '-c','8192', '-ngl','0',
        '--threads','4', '-np','1', '--jinja', '--alias',ENTRY['id'],
        '--chat-template-kwargs','{"enable_thinking":false}', '--perf', '--log-verbosity','4','--api-key',auth]
status = {'variant':VARIANT, 'state':'starting', 'start':stamp(), 'model':ENTRY,
          'scope':'rawHTTP outsideCore; syntheticpriorledger; no businessaction executed',
          'hypothesis':'minimal transport diagnostic after two first-generation deadlines; no ledger or productquality test',
          'fixture_sha256':hashlib.sha256(FIXTURE.read_bytes()).hexdigest(),
          'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
          'startup_s':60, 'first_generation_s':60, 'idle_generation_s':30,
          'turn_s':120, 'max_tokens':8, 'temperature':0, 'threads':4,
          'ping_setting':'unchanged server default',
          'server_args':[x if x != auth else '<ephemeral-test-auth-redacted>' for x in args],
          'results':[]}

def save():
    p = OUT/'status.tmp'
    p.write_text(json.dumps(status, ensure_ascii=False, indent=2)+'\n')
    p.replace(OUT/'status.json')

def server_log(stream):
    with (OUT/'server.log').open('w') as log:
        for line in iter(stream.readline, b''):
            log.write(line.decode('utf-8',errors='replace').replace(auth,'<ephemeral-test-auth-redacted>'))
            log.flush()

process = None
boot = time.monotonic()
save()
try:
    process = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
    status.update(pid=process.pid, group=process.pid,
                  birth=Path(f'/proc/{process.pid}/stat').read_text().rsplit(')',1)[1].split()[19])
    threading.Thread(target=server_log, args=(process.stdout,), daemon=True).start()
    save()
    while True:
        if process.poll() is not None:
            raise RuntimeError('own server exited before ready')
        try:
            c = http.client.HTTPConnection('127.0.0.1',port,timeout=2)
            c.request('GET','/health',headers={'Authorization':'Bearer '+auth})
            response = c.getresponse(); response.read(); ready = response.status == 200; c.close()
            if ready:
                break
        except OSError:
            pass
        if time.monotonic()-boot >= 60:
            raise TimeoutError('startup60s')
        time.sleep(.25)
    status.update(state='running', load_ms=round((time.monotonic()-boot)*1000)); save()
    cases = json.loads(FIXTURE.read_text())['results'][:1]
    for case in cases:
        body = request_body(case)
        (OUT/(case['scenario']+'-request.json')).write_text(json.dumps(body,ensure_ascii=False,indent=2)+'\n')
        result = {'scenario':case['scenario'], 'text':'', 'reasoning_chars':0, 'error':None,
                  'first_text_http_ms':None, 'first_generation_http_ms':None,
                  'finish_reasons':[], 'usage':None, 'events':[]}
        start = time.monotonic(); last_progress = None; buffer = b''; done = False
        c = http.client.HTTPConnection('127.0.0.1',port,timeout=2)
        try:
            c.connect(); c.sock.settimeout(60)
            c.request('POST','/v1/chat/completions',json.dumps(body).encode(),
                      headers={'Content-Type':'application/json','Authorization':'Bearer '+auth})
            response = c.getresponse(); result['http_status'] = response.status
            if response.status != 200:
                result['error_body'] = response.read(8192).decode('utf-8',errors='replace').replace(auth,'<ephemeral-test-auth-redacted>')
                raise RuntimeError('HTTP'+str(response.status))
            while not done:
                deadline = min(start+120, start+60 if last_progress is None else last_progress+30)
                now = time.monotonic()
                if now >= deadline:
                    raise TimeoutError('generation deadline; no retry')
                c.sock.settimeout(deadline-now)
                chunk = response.read1(65536)
                if not chunk:
                    break
                buffer += chunk
                while b'\n' in buffer:
                    line, buffer = buffer.split(b'\n',1)
                    if not line.startswith(b'data:'):
                        continue
                    payload = line[5:].strip()
                    if payload == b'[DONE]':
                        done = True; break
                    data = json.loads(payload); now = time.monotonic(); event = {'ms':round((now-start)*1000)}
                    if data.get('usage') is not None:
                        result['usage'] = data['usage']
                    for choice in data.get('choices',[]):
                        delta = choice.get('delta',{}); text = delta.get('content') or ''
                        reasoning = delta.get('reasoning_content') or delta.get('reasoning') or ''
                        tool = delta.get('tool_calls') or []
                        progress = bool(text or reasoning or delta.get('refusal') or any(t.get('function',{}).get('name') or t.get('function',{}).get('arguments') for t in tool))
                        if progress:
                            last_progress = now
                            if result['first_generation_http_ms'] is None:
                                result['first_generation_http_ms'] = event['ms']
                        if text:
                            result['text'] += text
                            if result['first_text_http_ms'] is None:
                                result['first_text_http_ms'] = event['ms']
                        result['reasoning_chars'] += len(reasoning)
                        event.update(text_chars=len(text), reasoning_chars=len(reasoning), tool_calls=tool)
                        if choice.get('finish_reason') is not None:
                            result['finish_reasons'].append(choice['finish_reason'])
                    result['events'].append(event)
        except (OSError,ValueError,RuntimeError) as error:
            result['error'] = type(error).__name__+': '+str(error)
        finally:
            c.close()
        result.update(elapsed_ms=round((time.monotonic()-start)*1000),
                      first_text_within_http60=result['first_text_http_ms'] is not None and result['first_text_http_ms']<=60000)
        status['results'].append(result); save()
        print(json.dumps({k:v for k,v in result.items() if k != 'events'},ensure_ascii=False),flush=True)
        if result['error']:
            break
except (OSError,RuntimeError) as error:
    status['error'] = type(error).__name__+': '+str(error)
finally:
    if process and process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill(); process.wait(); status['cleanup']='own server SIGTERM then SIGKILL'
    if process:
        status['server_exit']=process.returncode
    status.update(state='completed',end=stamp()); save()
