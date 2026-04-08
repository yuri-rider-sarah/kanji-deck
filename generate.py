#!/usr/bin/env python3

import genanki
import json
import sys

KANJI_MODEL_ID = 2138030364
KANJI_DECK_ID = 1896983062

with open('style.css') as f:
    css_style = f.read()

model = genanki.Model(
    model_id=KANJI_MODEL_ID,
    name='Kanji',
    fields=[
        {'name': 'Id'},
        {'name': 'Kanji'},
        {'name': 'Part'},
        {'name': 'Content'},
    ],
    templates=[
        {
            'name': 'Card 1',
            'qfmt': '<div class="q-kanji">{{Kanji}}</div><div class="part">{{Part}}</div>',
            'afmt': '{{FrontSide}}<div id="answer">{{Content}}</div>',
        },
        {
            'name': 'Card 2',
            'qfmt': '<div class="question">{{Content}}</div>',
            'afmt': '<div id="answer" class="q-kanji">{{Kanji}}</div><div class="part">{{Part}}</div>{{Content}}',
        },
    ],
    css=css_style,
)

deck = genanki.Deck(KANJI_DECK_ID, 'Kanji')

for entry in json.load(sys.stdin):
    entry_id = entry['kanji']
    if entry['part']:
        entry_id += ' (' + entry['part'] + ')'
    note = genanki.Note(model=model, fields=[entry_id, entry['kanji'], entry['part'], entry['content']])
    deck.add_note(note)

genanki.Package(deck).write_to_file('kanji.apkg')
