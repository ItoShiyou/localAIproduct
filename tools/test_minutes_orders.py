import os
from pathlib import Path
import tempfile
import unittest
from concurrent.futures import ThreadPoolExecutor
import subprocess
import sys
from minutes_orders import fulfill, block_order

class OrderTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='minutes-orders-')
        self.path = Path(self.temp.name) / 'orders.sqlite'
        self.request = {'licensee': 'テスト購入者', 'edition': 'pro_offline', 'public_key': 'test-only'}
        self.calls = 0
    def tearDown(self):
        self.temp.cleanup()
    def issue(self, request):
        self.calls += 1
        return 'MNT1-TEST-ONLY'
    def run_order(self, request=None, issue=None):
        return fulfill(self.path, 'test:order-1', request or self.request, issue or self.issue, True)
    def test_paid_confirmation_required_before_any_write(self):
        with self.assertRaises(ValueError):
            fulfill(self.path, 'test:order-1', self.request, self.issue)
        self.assertFalse(self.path.exists())
    def test_retry_returns_same_key_without_reissuing(self):
        self.assertTrue(self.run_order()[1])
        self.assertFalse(self.run_order()[1])
        self.assertEqual(self.calls, 1)
    def test_request_change_rejected(self):
        self.run_order()
        for field, value in [('licensee', '別人'), ('edition', 'pro'), ('public_key', 'other-key')]:
            with self.assertRaises(ValueError):
                self.run_order({**self.request, field: value})
        self.assertEqual(self.calls, 1)
    def test_issue_failure_can_retry(self):
        def failed(request):
            raise ValueError('simulated')
        with self.assertRaises(ValueError):
            self.run_order(issue=failed)
        self.assertTrue(self.run_order()[1])
    def test_refunded_order_cannot_be_redelivered(self):
        self.run_order()
        block_order(self.path, 'test:order-1')
        with self.assertRaises(ValueError):
            self.run_order()
    def test_invalid_inputs_and_git_boundary(self):
        with self.assertRaises(ValueError):
            fulfill(self.path, '../bad', self.request, self.issue, True)
        (self.path.parent / '.git').mkdir()
        with self.assertRaises(ValueError):
            self.run_order()
    def test_private_permissions(self):
        self.run_order()
        if os.name == 'posix':
            self.assertEqual(self.path.stat().st_mode & 0o777, 0o600)
    def test_malformed_issuer_output_is_not_saved(self):
        with self.assertRaises(ValueError):
            self.run_order(issue=lambda request: 'invalid')
        self.assertTrue(self.run_order()[1])
    def test_simultaneous_delivery_creates_only_one_key(self):
        with ThreadPoolExecutor(max_workers=2) as pool:
            results = list(pool.map(lambda _: self.run_order(), range(2)))
        self.assertEqual(self.calls, 1)
        self.assertEqual(sorted(created for _, created in results), [False, True])
    @unittest.skipUnless(os.environ.get('MINUTES_ISSUER_BIN'), 'requires built issuer executable; run with MINUTES_ISSUER_BIN')
    def test_real_cli_signs_verifies_retries_and_rejects_dev_in_production(self):
        key = Path(self.temp.name) / 'public-development-seed.key'
        key.write_text(b'minutes-DEV-ONLY-license-seed-01'.hex())
        if os.name == 'posix':
            key.chmod(0o600)
        script = Path(__file__).with_name('minutes_orders.py')
        arguments = [sys.executable, str(script), '--ledger', str(self.path), '--order-id', 'test:order-1',
            '--issuer', os.environ['MINUTES_ISSUER_BIN'], '--key', str(key), '--licensee', 'テスト専用', '--payment-confirmed']
        environment = {**os.environ, 'PYTHONIOENCODING': 'utf-8'}
        rejected = subprocess.run(arguments, capture_output=True, text=True, encoding='utf-8', env=environment)
        self.assertNotEqual(rejected.returncode, 0)
        self.assertFalse(self.path.exists())
        first = subprocess.run([*arguments, '--test-mode'], capture_output=True, text=True, encoding='utf-8', env=environment, check=True)
        second = subprocess.run([*arguments, '--test-mode'], capture_output=True, text=True, encoding='utf-8', env=environment, check=True)
        self.assertEqual(first.stdout.splitlines()[-1], second.stdout.splitlines()[-1])
        self.assertIn('既存キーを再交付', second.stdout)

if __name__ == '__main__':
    unittest.main()
