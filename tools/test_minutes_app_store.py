"""Distribution-profile failures must stop packaging, using synthetic metadata only."""
from datetime import datetime, timedelta, timezone
import unittest
import tempfile
import plistlib
from pathlib import Path
from minutes_app_store import validate_distribution_profile, validate

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

class BundleMetadataTests(unittest.TestCase):
    def test_unreviewed_model_or_audio_cannot_enter_distribution(self):
        from minutes_app_store import HASHES
        with tempfile.TemporaryDirectory() as root:
            app = Path(root)/'minutes.app'
            resources = app/'Contents/Resources'
            models = resources/'resources/models'
            models.mkdir(parents=True)
            info = {'CFBundleIdentifier': BUNDLE, 'MinutesDistributionChannel': 'mac-app-store',
                    'NSMicrophoneUsageDescription': '合成音声の検証',
                    'LSApplicationCategoryType': 'public.app-category.productivity'}
            (app/'Contents/Info.plist').write_bytes(plistlib.dumps(info))
            (resources/'PrivacyInfo.xcprivacy').write_bytes(plistlib.dumps({
                'NSPrivacyTracking': False, 'NSPrivacyTrackingDomains': [], 'NSPrivacyCollectedDataTypes': []}))
            for name in HASHES: (models/name).write_bytes(b'synthetic')
            for name in ['unknown.gguf', 'recording.wav', 'nested']:
                with self.subTest(name=name):
                    extra = models/name
                    extra.write_bytes(b'synthetic')
                    with self.assertRaisesRegex(ValueError, '同梱モデル'): validate(app)
                    extra.unlink()
            first = models/next(iter(HASHES))
            first.unlink()
            first.symlink_to(models/next(name for name in HASHES if name != first.name))
            with self.assertRaisesRegex(ValueError, '同梱モデル'): validate(app)

    def test_missing_store_category_fails_before_signing_or_model_reads(self):
        with tempfile.TemporaryDirectory() as root:
            app = Path(root)/'minutes.app'
            (app/'Contents').mkdir(parents=True)
            info = {'CFBundleIdentifier': BUNDLE, 'MinutesDistributionChannel': 'mac-app-store',
                    'NSMicrophoneUsageDescription': '合成音声の検証'}
            for category in [None, 'public.app-category.invalid']:
                with self.subTest(category=category):
                    info['LSApplicationCategoryType'] = category or ''
                    (app/'Contents/Info.plist').write_bytes(plistlib.dumps(info))
                    with self.assertRaisesRegex(ValueError, 'カテゴリ'):
                        validate(app)

    def test_tracking_or_collected_data_manifest_is_rejected_before_signing(self):
        with tempfile.TemporaryDirectory() as root:
            app = Path(root)/'minutes.app'
            (app/'Contents/Resources').mkdir(parents=True)
            info = {'CFBundleIdentifier': BUNDLE, 'MinutesDistributionChannel': 'mac-app-store',
                    'NSMicrophoneUsageDescription': '合成音声の検証',
                    'LSApplicationCategoryType': 'public.app-category.productivity'}
            (app/'Contents/Info.plist').write_bytes(plistlib.dumps(info))
            for field, value in [('NSPrivacyTracking', True),
                                 ('NSPrivacyTrackingDomains', ['example.invalid']),
                                 ('NSPrivacyCollectedDataTypes', [{'NSPrivacyCollectedDataType': 'test'}])]:
                with self.subTest(field=field):
                    manifest = {'NSPrivacyTracking': False, 'NSPrivacyTrackingDomains': [],
                                'NSPrivacyCollectedDataTypes': [], 'NSPrivacyAccessedAPITypes': []}
                    manifest[field] = value
                    (app/'Contents/Resources/PrivacyInfo.xcprivacy').write_bytes(plistlib.dumps(manifest))
                    with self.assertRaisesRegex(ValueError, 'PrivacyInfo'):
                        validate(app)

if __name__ == '__main__': unittest.main()
