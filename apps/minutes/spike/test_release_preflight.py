import unittest
from release_preflight import checks


class PreflightTests(unittest.TestCase):
    def test_missing_production_fields_are_blocked(self):
        result = checks({"bundle": {"copyright": "販売前に記入"}, "app": {"windows": [{"title": "仮称"}]}}, 'PRODUCTION_PUBLIC_KEY_HEX: &str = "";', "")
        self.assertTrue(all(not passed for passed in result.values()))

    def test_complete_local_fields_do_not_certify_external_checks(self):
        result = checks({"bundle": {"copyright": "© Example"}, "app": {"windows": [{"title": "minutes"}]}}, 'PRODUCTION_PUBLIC_KEY_HEX: &str = "' + "ab" * 32 + '";', "Whisper WeSpeaker Qwen MPL " * 100)
        self.assertTrue(all(result.values()))


if __name__ == "__main__":
    unittest.main()
