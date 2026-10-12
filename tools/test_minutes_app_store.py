"""Distribution-profile failures must stop packaging, using synthetic metadata only."""
from datetime import datetime, timedelta, timezone
import unittest
from minutes_app_store import validate_distribution_profile

NOW = datetime(2026, 10, 13, tzinfo=timezone.utc)
BUNDLE = 'dev.localaiproduct.minutes'

class ProfileTests(unittest.TestCase):
    def profile(self):
        return {'ExpirationDate': (NOW + timedelta(days=30)).replace(tzinfo=None),
                'TeamIdentifier': ['TESTTEAM'],
                'Entitlements': {'com.apple.application-identifier': 'TESTTEAM.'+BUNDLE,
                                 'com.apple.developer.team-identifier': 'TESTTEAM'}}

    def test_accepts_valid_explicit_distribution_profile(self):
        document = self.profile()
        self.assertEqual(validate_distribution_profile(document, BUNDLE, NOW), document['Entitlements'])

    def test_rejects_expiry_and_missing_expiry(self):
        for expiry in [NOW - timedelta(seconds=1), NOW, None]:
            with self.subTest(expiry=expiry):
                document = self.profile(); document['ExpirationDate'] = expiry
                with self.assertRaises(ValueError): validate_distribution_profile(document, BUNDLE, NOW)

    def test_rejects_other_app_and_wildcard(self):
        for appid in ['TESTTEAM.another.app', 'TESTTEAM.*', 'TESTTEAM.*.'+BUNDLE]:
            with self.subTest(appid=appid):
                document = self.profile(); document['Entitlements']['com.apple.application-identifier'] = appid
                with self.assertRaises(ValueError): validate_distribution_profile(document, BUNDLE, NOW)

    def test_rejects_debug_and_device_distribution(self):
        for key in ['get-task-allow', 'com.apple.security.get-task-allow', 'ProvisionedDevices', 'ProvisionsAllDevices']:
            with self.subTest(key=key):
                document = self.profile()
                target = document['Entitlements'] if 'task-allow' in key else document
                target[key] = ['TESTDEVICE'] if key == 'ProvisionedDevices' else True
                with self.assertRaises(ValueError): validate_distribution_profile(document, BUNDLE, NOW)

    def test_rejects_missing_or_mismatched_team(self):
        for team in [None, 'OTHERTEAM']:
            with self.subTest(team=team):
                document = self.profile(); document['Entitlements']['com.apple.developer.team-identifier'] = team
                with self.assertRaises(ValueError): validate_distribution_profile(document, BUNDLE, NOW)

if __name__ == '__main__': unittest.main()
