import unittest
from release_preflight import mac_permissions

class PermissionTests(unittest.TestCase):
    def test_valid_configuration(self):
        self.assertTrue(mac_permissions({'bundle':{'macOS':{'hardenedRuntime':True,'entitlements':'Entitlements.plist'}}}, {'com.apple.security.device.audio-input':True}))
    def test_missing_audio_permission_is_blocked(self):
        self.assertFalse(mac_permissions({'bundle':{'macOS':{'hardenedRuntime':True,'entitlements':'Entitlements.plist'}}}, {}))
    def test_missing_runtime_and_wrong_file_are_blocked(self):
        for mac in [{}, {'hardenedRuntime':False,'entitlements':'Entitlements.plist'}, {'hardenedRuntime':True,'entitlements':'other.plist'}]:
            self.assertFalse(mac_permissions({'bundle':{'macOS':mac}}, {'com.apple.security.device.audio-input':True}))

if __name__ == '__main__':
    unittest.main()
