# SPDX-License-Identifier: MPL-2.0
from copy import deepcopy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "mods/expedition"))
from native_campaign import Campaign


class HeorotTowerTests(unittest.TestCase):
    def test_return_door_uses_the_story_tower_after_harold(self):
        tower = {
            "class": "ZoneChangeMenuItem", "position": "{22,12.5}", "iconSpriteFile": "storyIcon.png",
            "conditions": {"finishedQuests": ["Finam.StrikeHeorot"],
                           "andCondition": {"finishedQuests": ["!Heorot.SaveBeo"]}},
            "arguments": {"zoneChange": {"type": "Generic", "arguments": {
                "dungeon": {"type": "TiledDungeon", "arguments": "HeorotDungeon.plist"},
                "targetZoneChange": "RoyalDungeonsEntrance"}}},
        }
        grove = deepcopy(tower)
        grove["arguments"]["zoneChange"]["arguments"]["dungeon"]["arguments"] = "ArborunDungeon.plist"
        buildings = [deepcopy(tower)]
        resources = {"Towns": {"over-87x100": {"buildings": buildings},
                               "over-123x110": {"buildings": [grove]}},
                     "Quests": {}, "Chats": {}, "Overworld": {"contentProviderArguments": {}}}
        Campaign({"branches": [], "quests": []}, resources, [], None).install_map()
        story, after_harold = buildings
        self.assertEqual(story, tower)
        self.assertEqual(after_harold["conditions"], {"finishedQuests": ["Heorot.CaptureTheKing"]})
        self.assertEqual(after_harold["position"], tower["position"])
        self.assertEqual(after_harold["arguments"], tower["arguments"])
        self.assertNotIn("enterEvents", after_harold)
