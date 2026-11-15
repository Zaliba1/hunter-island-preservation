# SPDX-License-Identifier: MPL-2.0
"""Game language choices; independent of the launcher's interface language."""
LANGUAGES = (
    ('en', 'English'),
    ('de', 'Deutsch'),
    ('fr', 'Français'),
    ('es', 'Español'),
    ('it', 'Italiano'),
    ('ja', '日本語'),
    ('system', 'Systemsprache / System language'),
)


def selected_language(settings):
    code = settings.get('language', 'en')
    return code if code in tuple(code for code, _ in LANGUAGES) else 'en'


def language_arguments(settings):
    code = selected_language(settings)
    return [] if code == 'system' else ['--preferred-languages=' + code]
