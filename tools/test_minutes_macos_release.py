from pathlib import Path
import plistlib
import tempfile
import unittest
from unittest.mock import patch
import minutes_macos_release as release

class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='minutes-release-test-')
        self.root = Path(self.temp.name)
        self.app = self.root / 'minutes.app'
        (self.app / 'Contents').mkdir(parents=True)
        (self.app / 'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier':'dev.localaiproduct.minutes','CFBundleShortVersionString':'0.1.0'}))
        self.output = self.root / 'output'
        self.calls = []
    def tearDown(self):
        self.temp.cleanup()
    def simulate(self,arguments):
        self.calls.append(arguments)
        if arguments[:2] == ['ditto','-c']:
            Path(arguments[-1]).write_bytes(b'test-only artifact')
        if arguments[:3] == ['xcrun','notarytool','submit']:
            return '{"id":"test-only","status":"Accepted"}'
        return ''
    def test_missing_production_configuration_blocks_before_copy_or_upload(self):
        with patch.object(release,'checks',return_value={'production':False}):
            with self.assertRaises(ValueError):
                release.execute(self.app,self.output,'Developer ID Application: Test','test-profile',self.simulate)
        self.assertFalse(self.output.exists())
        self.assertFalse(self.calls)
    def test_owner_profile_flow_and_manifest_with_mocked_apple_tools(self):
        original = (self.app / 'Contents/Info.plist').read_bytes()
        with patch.object(release,'checks',return_value={'test-only':True}):
            report = release.execute(self.app,self.output,'Developer ID Application: Test','test-profile',self.simulate)
        self.assertEqual(report['notarization_id'],'test-only')
        self.assertEqual(len(report['sha256']),64)
        self.assertEqual((self.app / 'Contents/Info.plist').read_bytes(),original)
        self.assertIn(['xcrun','stapler','validate',str(self.output / 'staging/minutes.app')],self.calls)
    def test_rejected_notarization_never_creates_release_zip(self):
        def rejected(arguments):
            if arguments[:3] == ['xcrun','notarytool','submit']:
                return '{"status":"Invalid"}'
            return self.simulate(arguments)
        with patch.object(release,'checks',return_value={'test-only':True}):
            with self.assertRaises(ValueError):
                release.execute(self.app,self.output,'Developer ID Application: Test','test-profile',rejected)
        self.assertFalse((self.output / 'minutes-0.1.0-macos-arm64.zip').exists())
    def test_ad_hoc_and_existing_output_are_rejected(self):
        with self.assertRaises(ValueError):
            release.execute(self.app,self.output,'-', 'test-profile',self.simulate)
        self.output.mkdir()
        with self.assertRaises(ValueError):
            release.execute(self.app,self.output,'Developer ID Application: Test','test-profile',self.simulate)
    def test_output_inside_source_bundle_is_rejected(self):
        with self.assertRaises(ValueError):
            release.execute(self.app,self.app/'output','Developer ID Application: Test','test-profile',self.simulate)
        self.assertFalse((self.app/'output').exists())
        self.assertFalse(self.calls)
    def test_source_text_is_read_as_utf8_on_every_platform(self):
        original_read = Path.read_text
        def utf8_only(path, *arguments, **keywords):
            self.assertEqual(keywords.get('encoding'), 'utf-8')
            return original_read(path, *arguments, **keywords)
        with patch.object(Path, 'read_text', utf8_only), patch.object(release, 'checks', return_value={'test-only':True}):
            release.execute(self.app,self.output,'Developer ID Application: Test','test-profile',self.simulate)
        self.assertTrue(self.calls)

    def test_separate_editions_keep_names_and_do_not_require_activation_keys(self):
        for edition in ('free', 'pro'):
            app = self.root / f'minutes {edition.title()}.app'
            app.mkdir()
            import shutil
            shutil.copytree(self.app / 'Contents', app / 'Contents')
            output = self.root / edition
            with patch.object(release, 'validate_edition_resources'), patch.object(release, 'checks', return_value={'test-only':True}) as check:
                report = release.execute(app, output, 'Developer ID Application: Test', 'test-profile', self.simulate)
            self.assertEqual(check.call_args.args[3], edition)
            self.assertEqual(report['edition'], edition)
            self.assertEqual(report['file'], f'minutes-{edition}-0.1.0-macos-arm64.zip')
            self.assertTrue((output / 'staging' / app.name).exists())

    def test_missing_models_and_paid_assets_in_free_are_rejected(self):
        with self.assertRaises(ValueError):
            release.validate_edition_resources(self.app, 'pro')
        models = self.app / 'Contents/Resources/resources/models'
        models.mkdir(parents=True)
        (models / 'unexpected.gguf').write_bytes(b'test-only')
        with self.assertRaises(ValueError):
            release.validate_edition_resources(self.app, 'free')

    def test_wrong_model_hash_is_rejected(self):
        models = self.app / 'Contents/Resources/resources/models'
        models.mkdir(parents=True)
        (models / 'ggml-small-q5_1.bin').write_bytes(b'not a model')
        with self.assertRaises(ValueError):
            release.validate_edition_resources(self.app, 'free')

if __name__ == '__main__':
    unittest.main()
