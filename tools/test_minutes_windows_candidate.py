import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile
import minutes_windows_candidate as candidate

class CandidateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='minutes-windows-package-test-')
        self.root = Path(self.temp.name)
        self.exe, self.sidecar = self.root/'minutes.exe', self.root/'sidecar.exe'
        self.exe.write_bytes(b'MZtest-main'); self.sidecar.write_bytes(b'MZtest-sidecar')
        self.resources = self.root/'resources'
        (self.resources/'models').mkdir(parents=True)
        (self.resources/'onnxruntime').mkdir()
        (self.resources/'onnxruntime/onnxruntime.dll').write_bytes(b'MZtest-runtime')
        (self.resources/'THIRD_PARTY_NOTICES.txt').write_text('test-only notices', encoding='utf-8')
        self.hashes = {}
        for name in candidate.MODEL_HASHES:
            (self.resources/'models'/name).write_bytes(b'test-only-model')
            self.hashes[name] = hashlib.sha256(b'test-only-model').hexdigest()
        self.output = self.root/'output'
    def tearDown(self):
        self.temp.cleanup()
    def run_package(self):
        return candidate.package(self.exe, self.resources, self.sidecar, self.output)
    def test_complete_candidate_includes_all_models_and_is_not_sales_ready(self):
        with patch.object(candidate, 'MODEL_HASHES', self.hashes):
            report = self.run_package()
        self.assertFalse(report['salesReady']); self.assertFalse(report['signed'])
        with zipfile.ZipFile(self.output/report['file']) as archive:
            self.assertIn('minutes Pro/resources/models/Qwen3-4B-Instruct-2507-Q4_K_M.gguf', archive.namelist())
            self.assertIn('minutes Pro/TEST-CANDIDATE.txt', archive.namelist())
        self.assertEqual(report['sha256'], candidate.digest(self.output/report['file']))
    def test_wrong_hash_fails_before_creating_output(self):
        with self.assertRaises(ValueError): self.run_package()
        self.assertFalse(self.output.exists())
    def test_missing_engine_fails_before_creating_output(self):
        self.sidecar.unlink()
        with self.assertRaises(ValueError): self.run_package()
        self.assertFalse(self.output.exists())
    def test_existing_output_is_preserved(self):
        self.output.mkdir(); marker=self.output/'keep'; marker.write_bytes(b'keep')
        with self.assertRaises(ValueError): self.run_package()
        self.assertEqual(marker.read_bytes(), b'keep')
    def test_non_windows_binary_rejected(self):
        self.exe.write_bytes(b'not PE')
        with self.assertRaises(ValueError): self.run_package()
        self.assertFalse(self.output.exists())

if __name__ == '__main__': unittest.main()
