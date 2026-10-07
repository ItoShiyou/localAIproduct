"""Run three separate compiled-edition processes on fictional local data.
No real recognizer, microphone, Keychain, downloads or user records are used.
"""
import os
from pathlib import Path
import subprocess
import tempfile

REPO = Path(__file__).resolve().parents[1]

def main():
    with tempfile.TemporaryDirectory(prefix='minutes-upgrade-test-') as folder:
        fixture = Path(folder)
        (fixture / 'fixture-only').write_text('fictional-test-data', encoding='utf-8')
        for edition, phase in [('free', 'create'), ('pro', 'pro'), ('free', 'return-free')]:
            print(f'{edition}: {phase}', flush=True)
            env = dict(os.environ, MINUTES_UPGRADE_FIXTURE=folder, MINUTES_UPGRADE_PHASE=phase)
            subprocess.run(['cargo', 'test', '--no-default-features', '--features', f'edition-{edition}',
                '--test', 'distribution_editions', 'persisted_edition_upgrade', '--', '--ignored', '--exact'],
                cwd=REPO / 'apps/minutes/src-tauri', env=env, check=True)
    print('PASS: Free → Pro → Free preserved audio, edits, notes, metadata, search, tags and usage; edition export gates enforced.')

if __name__ == '__main__':
    main()
