"""Localize the expedition's own strings before the retail language tables are packed."""
import json
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
LANGUAGES = ('en', 'fr', 'es', 'it', 'ja')


def expedition_tables(strings):
    catalogue = json.loads((HERE / 'quest-locales.json').read_text('utf-8'))
    battle = json.loads((HERE / 'battle-locales.json').read_text('utf-8'))
    tables = {'de': dict(strings), **{lang: {} for lang in LANGUAGES}}
    for key, source in strings.items():
        row = catalogue.get(source)
        if not isinstance(row, list) or len(row) != len(LANGUAGES):
            raise ValueError(f'Missing expedition translation for {key}: {source}')
        for lang, translated in zip(LANGUAGES, row):
            if (not translated.strip() or source.count('&@') != translated.count('&@')
                    or re.findall(r'%[@d]', source) != re.findall(r'%[@d]', translated)):
                raise ValueError(f'Invalid expedition string: {lang}/{key}')
            tables[lang][key] = translated
    for key, row in battle.items():
        if key in strings or set(row) != tables.keys():
            raise ValueError(f'Invalid battle string: {key}')
        for lang, translated in row.items():
            if not translated.strip() or sorted(re.findall(r'\{\d+\}', row['de'])) != sorted(re.findall(r'\{\d+\}', translated)):
                raise ValueError(f'Invalid battle placeholders: {lang}/{key}')
            tables[lang][key] = translated
    return tables
