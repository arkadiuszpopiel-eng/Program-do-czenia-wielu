"""Odtworzenie syntetycznych ciał HTTP; diagnostyka modelu, nie nowy próg evals."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import time
import urllib.request

ROOT = Path('/workspace/scratch/wire-diagnosis')
DEADLINE = datetime.datetime(2026, 11, 9, 7, tzinfo=datetime.timezone.utc)
MODEL = '/workspace/models/Bielik-1.5B-v3.0-Instruct.Q8_0.gguf'
ALIAS = 'bielik-1.5b-v3.0-instruct-q8_0'
# Publiczna stała fixture, nie sekret użytkownika ani klucz z konfiguracji Alfy.
FIXTURE_KEY = 'public-synthetic-fixture-localhost-only'


def now():
    return datetime.datetime.now(datetime.timezone.utc)


def main():
    if now() >= DEADLINE:
        raise SystemExit('Minął termin; nie uruchamiam próby')
    bodies = [json.loads(s) for s in (ROOT / 'requests.jsonl').read_text().splitlines()]
    assert len(bodies) == 50
    for body in bodies:
        assert body['max_tokens'] == 96 and body['temperature'] == 0
        assert set(body) == {'model', 'messages', 'stream', 'stream_options', 'max_tokens', 'temperature'}
        assert [m['role'] for m in body['messages']] == ['system', 'user']
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    endpoint = f'http://127.0.0.1:{port}'
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def post(path, body):
        return opener.open(urllib.request.Request(
            endpoint + path, data=json.dumps(body, ensure_ascii=False).encode(),
            headers={'Content-Type': 'application/json', 'Authorization': 'Bearer ' + FIXTURE_KEY}), timeout=120)

    args = ['/workspace/tools/llama/llama-b11559/llama-server', '--host', '127.0.0.1',
            '--port', str(port), '--api-key', FIXTURE_KEY, '-m', MODEL,
            '--alias', ALIAS, '-c', '8192', '-ngl', '0', '--threads', '4', '-np', '1']
    started = now()
    rows = []
    report = {'started_utc': started.isoformat(), 'platform': 'Linux Cloud CPU; direct HTTP replay, no app decoder/microphone',
              'server_build': 'b11559', 'model': ALIAS, 'model_sha256': 'e206ebbd9ee00cc8f14af5a096ad3f193a59608c63905316e8dc680c96655a60',
              'source': 'HTTP bodies from existing app-core tests on fake-llama-server', 'max_tokens': 96, 'temperature': 0,
              'tool_capability': False, 'results': rows}
    with (ROOT / 'server.log').open('w') as log:
        server = subprocess.Popen(args, stdout=log, stderr=subprocess.STDOUT)
        (ROOT / 'server.pid').write_text(str(server.pid))
        try:
            for _ in range(600):
                if server.poll() is not None:
                    raise RuntimeError('serwer zakończył start: ' + str(server.returncode))
                try:
                    with opener.open(endpoint + '/health', timeout=2) as response:
                        if response.status == 200:
                            break
                except OSError:
                    time.sleep(0.1)
            else:
                raise RuntimeError('brak gotowości w60s')
            templates = []
            for idx in [40, 45]:
                body = bodies[idx]
                with post('/apply-template', {'messages': body['messages'], 'add_generation_prompt': True}) as response:
                    template = json.load(response)
                prompt = template['prompt']
                assert all(message['content'] in prompt for message in body['messages'])
                templates.append({'mode': 'standard' if idx == 40 else 'jarvis', 'prompt': prompt})
            (ROOT / 'templates.json').write_text(json.dumps(templates, ensure_ascii=False, indent=2) + '\n')
            for i, body in enumerate(bodies):
                if (DEADLINE - now()).total_seconds() < 130:
                    raise RuntimeError('przerwa przed terminem')
                if i < 40:
                    agent = ['alfa', 'beta', 'gama', 'delta'][i % 4]
                    mode = 'standard' if i < 20 else 'jarvis'
                    repeat = (i % 20) // 4 + 1
                else:
                    agent, mode, repeat = 'luna', 'standard' if i < 45 else 'jarvis', (i - 40) % 5 + 1
                text, finish, usage = '', None, None
                tick = time.monotonic()
                with post('/v1/chat/completions', body) as response:
                    for line in response:
                        if not line.startswith(b'data: '):
                            continue
                        value = line[6:].strip()
                        if value == b'[DONE]':
                            break
                        event = json.loads(value)
                        if event.get('usage'):
                            usage = event['usage']
                        for choice in event.get('choices', []):
                            text += choice.get('delta', {}).get('content') or ''
                            finish = choice.get('finish_reason') or finish
                assert text.strip() and finish == 'stop', (agent, mode, finish)
                rows.append({'agent': agent, 'mode': mode, 'repeat': repeat, 'text': text,
                             'finish_reason': finish, 'usage': usage, 'elapsed_ms': round((time.monotonic() - tick) * 1000),
                             'body_sha256': hashlib.sha256(json.dumps(body, ensure_ascii=False, sort_keys=True).encode()).hexdigest()})
                report['updated_utc'] = now().isoformat()
                (ROOT / 'direct-results.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
                print(f'{i+1}/50 {agent}/{mode}/{repeat}: {text}', flush=True)
            report['finished_utc'] = now().isoformat()
            report['completed'] = True
            (ROOT / 'direct-results.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
        finally:
            server.terminate()
            try:
                server.wait(timeout=15)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
            (ROOT / 'server.pid').unlink(missing_ok=True)


if __name__ == '__main__':
    main()
