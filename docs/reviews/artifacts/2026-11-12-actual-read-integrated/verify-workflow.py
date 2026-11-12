"""Kontrola zakresu zmiany względem a2b47cf2; wymaga PyYAML, bez testu PowerShell."""
import copy
import json
import pathlib
import subprocess
import sys

import yaml

base = sys.argv[1]
root = pathlib.Path(sys.argv[2]).resolve()
path = '.github/workflows/rehearsal.yml'
before = yaml.load(subprocess.check_output(['git', 'show', base + ':' + path], cwd=root).decode(), Loader=yaml.BaseLoader)
after = yaml.load((root / path).read_text(), Loader=yaml.BaseLoader)
normalized = copy.deepcopy(after)
original_steps = before['jobs']['live-engines']['steps']
for job in normalized['jobs'].values():
    for step in job.get('steps', []):
        name = step.get('name')
        if name == 'Build ekranu Core na rzeczywistym modelu':
            assert step.pop('id') == 'build_records'
        elif name == 'Rzeczywisty odczyt TEMP — kontrola Core i Qwen8B':
            assert step.pop('id') == 'actual_read_control'
            assert step['if'] == "${{ !cancelled() && github.event_name != 'schedule' && steps.build_records.outcome == 'success' }}"
            step['if'] = "github.event_name != 'schedule'"
        elif name == 'Odpowiedź po rzeczywistym odczycie — dane do przeglądu':
            assert step.pop('env') == {'ALFA_ACTUAL_READ_STEP_OUTCOME': '${{ steps.actual_read_control.outcome }}'}
            step['run'] = next(s['run'] for s in original_steps if s.get('name') == name)
assert normalized == before, 'nieoczekiwana zmiana komend, limitów, wyzwalaczy lub innych zadań'
print(json.dumps({'base': base, 'yaml': 'valid', 'structural_comparison': 'passed', 'scope': 'No PowerShell execution or model acceptance'}, indent=2))
