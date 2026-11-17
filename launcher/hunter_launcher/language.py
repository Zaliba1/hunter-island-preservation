# SPDX-License-Identifier: MPL-2.0
"""One saved selection for the Windows launcher and the original game."""
from functools import cache
import json
import os
from pathlib import Path
from plistlib import InvalidFileException
from zipfile import BadZipFile

LANGUAGES = (
    ('en', 'English'),
    ('de', 'Deutsch'),
    ('fr', 'Français'),
    ('es', 'Español'),
    ('it', 'Italiano'),
    ('ja', '日本語'),
    ('system', 'System language'),
)
LANGUAGE_CODES = tuple(code for code, _ in LANGUAGES)
GAME_LANGUAGES = frozenset(LANGUAGE_CODES) - {'system'}


def selected_language(settings):
    code = settings.get('language', 'en')
    return code if code in LANGUAGE_CODES else 'en'


def windows_ui_languages():
    if os.name != 'nt':
        return ()
    import ctypes
    from ctypes import wintypes

    get_languages = ctypes.windll.kernel32.GetUserPreferredUILanguages
    get_languages.argtypes = (wintypes.DWORD, ctypes.POINTER(wintypes.ULONG),
                              wintypes.LPWSTR, ctypes.POINTER(wintypes.ULONG))
    get_languages.restype = wintypes.BOOL
    count = wintypes.ULONG()
    length = wintypes.ULONG()
    # Flag 0 requests the default language-name format, for example es-MX.
    if not get_languages(0, ctypes.byref(count), None, ctypes.byref(length)) or not length.value:
        return ()
    names = ctypes.create_unicode_buffer(length.value)
    if not get_languages(0, ctypes.byref(count), names, ctypes.byref(length)):
        return ()
    return names[:length.value].split('\0')[:count.value]


def active_language(settings):
    code = selected_language(settings)
    if code != 'system':
        return code
    for tag in windows_ui_languages():
        code = tag.split('-', 1)[0].lower()
        if code in GAME_LANGUAGES:
            return code
    return 'en'


def language_arguments(settings):
    return ['--preferred-languages=' + active_language(settings)]


@cache
def translations(code):
    path = Path(__file__).with_name('translations') / (code + '.json')
    if code not in GAME_LANGUAGES:
        raise ValueError('Unknown launcher language')
    return json.loads(path.read_text(encoding='utf-8'))


def message(code, key, **values):
    return translations(code)[key].format(**values)


class LauncherIssue(Exception):
    def __init__(self, key, **values):
        self.key = key
        self.values = values
        super().__init__(message('en', key, **values))


class LauncherInputError(LauncherIssue, ValueError):
    pass


class LauncherRuntimeError(LauncherIssue, RuntimeError):
    pass


def issue_message(code, error):
    if isinstance(error, LauncherIssue):
        return message(code, error.key, **error.values)
    if isinstance(error, (BadZipFile, InvalidFileException)):
        return message(code, 'invalid_ipa')
    return str(error)
