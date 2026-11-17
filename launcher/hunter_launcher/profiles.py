# SPDX-License-Identifier: MPL-2.0
"""Fixed, independent save collections selected by mutually exclusive options."""
TEST_PROFILES = {
    '--hunter-test-save': 'pockettrend.hunterisland-test-v1',
    '--hunter-level1-save': 'pockettrend.hunterisland-level1-v1',
    '--hunter-triple99-save': 'pockettrend.hunterisland-triple99-v1',
}

def selected_test_directory(settings):
    mods = settings.get('mods', {})
    if not isinstance(mods, dict) or any(not isinstance(value, bool) for value in mods.values()):
        raise ValueError('Invalid mod settings')
    choices = [folder for flag, folder in TEST_PROFILES.items() if mods.get(flag, False)]
    if len(choices) > 1:
        raise ValueError('Choose only one collection mode')
    return choices[0] if choices else None
