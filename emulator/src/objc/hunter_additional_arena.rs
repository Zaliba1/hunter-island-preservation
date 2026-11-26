// SPDX-License-Identifier: MPL-2.0
//! Optional guest-native arena encounters. Party members here are fresh, battle-only monsters.
use super::{autorelease, hunter_battle_ai, hunter_silver_exchange, id, msg, msg_class, nil, ObjC, SEL};
use crate::abi::GuestFunction;
use crate::frameworks::core_graphics::{CGPoint, CGSize};
use crate::frameworks::foundation::ns_string::{from_rust_string, get_static_str, to_rust_string};
use crate::mem::Ptr;
use crate::Environment;
use serde::Deserialize;
use std::{collections::{BTreeMap, HashSet}, sync::OnceLock};

const TOURNAMENT_ENTER: u32 = 0x12885d;
const SETUP_ENEMY_TIER: u32 = 0xbceb9;
const RESULT_WIN: u32 = 0x96985;
const RESULT_LOSS: u32 = 0x97185;
const END_ENCOUNTER: u32 = 0x97b59;
const DEALLOC_TOWN_ENCOUNTER: u32 = 0x17f721;
const MENU_TAG: i32 = 0x4849_4152;
const LEVEL: u32 = 75;
const RANK_S: u32 = 5;
const TIER_IVAR: u32 = 340; // Living.tier in the checked Hunter Island 2.02 image.
const OPEN: &str = "touchHLEHunterOpenAdditionalArena:";
const CLOSE: &str = "touchHLEHunterCloseAdditionalArena:";
const SELECT: &str = "touchHLEHunterSelectAdditionalArenaTeam:";

#[derive(Default)]
pub(super) struct State {
    encounter: Option<id>,
    members: HashSet<id>,
}

#[derive(Deserialize)]
struct Arena {
    teams: Vec<Team>,
    locales: BTreeMap<String, Text>,
}

#[derive(Deserialize)]
struct Team {
    id: String,
    roster: Vec<String>,
}

#[derive(Deserialize)]
struct Text {
    title: String,
    choose: String,
    close: String,
    empty: String,
    failed: String,
    teams: Vec<String>,
}

fn data() -> &'static Arena {
    static ARENA: OnceLock<Arena> = OnceLock::new();
    ARENA.get_or_init(|| serde_json::from_str(include_str!("../../../mods/additional-arena/arena.json")).unwrap())
}

fn locale(env: &mut Environment) -> &'static Text {
    let languages: id = msg_class![env; NSLocale preferredLanguages];
    let count: u32 = msg![env; languages count];
    for i in 0..count {
        let language: id = msg![env; languages objectAtIndex:i];
        let tag = to_rust_string(env, language);
        if let Some(text) = tag.split(['-', '_']).next().and_then(|code| data().locales.get(&code.to_ascii_lowercase())) {
            return text;
        }
    }
    &data().locales["en"]
}

fn text(env: &mut Environment, value: impl Into<String>) -> id {
    let value = from_rust_string(env, value.into());
    autorelease(env, value)
}

fn put(env: &mut Environment, dictionary: id, key: &'static str, value: id) {
    let key = get_static_str(env, key);
    () = msg![env; dictionary setObject:value forKey:key];
}

pub(super) fn is_current(env: &Environment, encounter: id) -> bool {
    env.objc.hunter_additional_arena.encounter == Some(encounter)
}

pub(super) fn active(env: &Environment) -> bool {
    env.objc.hunter_additional_arena.encounter.is_some()
}

pub(super) fn before_guest_call(env: &mut Environment, receiver: id, imp: GuestFunction) -> bool {
    match imp.addr_with_thumb_bit() {
        SETUP_ENEMY_TIER if env.objc.hunter_additional_arena.members.contains(&receiver) => true,
        RESULT_WIN | RESULT_LOSS if is_current(env, receiver) => {
            env.objc.hunter_additional_arena.encounter = None;
            env.objc.hunter_additional_arena.members.clear();
            if imp.addr_with_thumb_bit() == RESULT_WIN {
                () = msg![env; receiver combatVictoryFinished:nil];
            } else {
                () = msg![env; receiver combatLossFinished:nil];
            }
            true
        }
        END_ENCOUNTER | DEALLOC_TOWN_ENCOUNTER if is_current(env, receiver) => {
            env.objc.hunter_additional_arena.encounter = None;
            env.objc.hunter_additional_arena.members.clear();
            false
        }
        _ => false,
    }
}

pub(super) fn after_guest_call(env: &mut Environment, node: id, imp: GuestFunction) {
    if imp.addr_with_thumb_bit() != TOURNAMENT_ENTER || !hunter_battle_ai::compatible(env) {
        return;
    }
    let existing: id = msg![env; node getChildByTag:MENU_TAG];
    if existing != nil {
        return;
    }
    let class = ObjC::read_isa(node, &env.mem);
    let metaclass = ObjC::read_isa(class, &env.mem);
    let callback = &(menu_action as fn(&mut Environment, id, SEL, id));
    let open = hunter_silver_exchange::register_action(env, metaclass, OPEN, b"v12@0:4@8", callback);
    for action in [SELECT, CLOSE] {
        hunter_silver_exchange::register_action(env, metaclass, action, b"v12@0:4@8", callback);
    }

    let strings = locale(env);
    let label = text(env, strings.title.as_str());
    let button: id = msg_class![env; CCMenuItemFont itemWithString:label target:class selector:open];
    let director: id = msg_class![env; CCDirector sharedDirector];
    let size: CGSize = msg![env; director winSize];
    () = msg![env; button setFontSize:((size.width * 0.028).round() as u32)];
    let bounds: CGSize = msg![env; button contentSize];
    let padding = size.width * 0.009;
    let brown = u32::from_le_bytes([92, 50, 21, 245]);
    let background: id = msg_class![env; CCLayerColor layerWithColor:brown
        width:(bounds.width + 2.0 * padding) height:(bounds.height + padding)];
    () = msg![env; background setPosition:(CGPoint { x: -padding, y: -padding * 0.5 })];
    () = msg![env; button addChild:background z:(-1i32)];
    let buttons: id = msg_class![env; NSArray arrayWithObject:button];
    let menu: id = msg_class![env; CCMenu menuWithArray:buttons];
    () = msg![env; menu setPosition:(CGPoint { x: size.width * 0.50, y: size.height * 0.08 })];
    () = msg![env; node addChild:menu z:20i32 tag:MENU_TAG];
    log!("Additional Arena Fights menu ready");
}

pub(super) fn register_menu_touch(env: &mut Environment, node: id, imp: GuestFunction) -> bool {
    hunter_silver_exchange::register_tagged_menu_touch(env, node, imp, MENU_TAG)
}

fn show_opponents(env: &mut Environment, target: id) {
    let strings = locale(env);
    let director: id = msg_class![env; CCDirector sharedDirector];
    let size: CGSize = msg![env; director winSize];
    let scene: id = msg_class![env; CCScene node];
    let asset = get_static_str(env, "ArenaCombat.jpg");
    let picture: id = msg_class![env; CCSprite spriteWithFile:asset];
    if picture != nil {
        let bounds: CGSize = msg![env; picture contentSize];
        () = msg![env; picture setScaleX:(size.width / bounds.width)];
        () = msg![env; picture setScaleY:(size.height / bounds.height)];
        () = msg![env; picture setPosition:(CGPoint { x: size.width * 0.5, y: size.height * 0.5 })];
        () = msg![env; scene addChild:picture];
    }
    let shade = u32::from_le_bytes([13, 11, 9, 229]);
    let backdrop: id = msg_class![env; CCLayerColor layerWithColor:shade width:(size.width) height:(size.height)];
    () = msg![env; scene addChild:backdrop];
    let font = get_static_str(env, "Helvetica");
    for (content, y, scale) in [(&strings.title, 0.92, 0.037), (&strings.choose, 0.85, 0.021)] {
        let content = text(env, content.as_str());
        let label: id = msg_class![env; CCLabelTTF labelWithString:content fontName:font fontSize:(size.width * scale)];
        () = msg![env; label setPosition:(CGPoint { x: size.width * 0.5, y: size.height * y })];
        () = msg![env; scene addChild:label];
    }
    let buttons: id = msg_class![env; NSMutableArray array];
    let choose = env.objc.lookup_selector(SELECT).unwrap();
    let close = env.objc.lookup_selector(CLOSE).unwrap();
    for (index, name) in strings.teams.iter().chain([&strings.close]).enumerate() {
        let label = text(env, name.as_str());
        let action = if index == strings.teams.len() { close } else { choose };
        let button: id = msg_class![env; CCMenuItemFont itemWithString:label target:target selector:action];
        () = msg![env; button setFontSize:((size.height * 0.038).round() as u32)];
        () = msg![env; button setTag:(index as i32)];
        () = msg![env; buttons addObject:button];
    }
    let menu: id = msg_class![env; CCMenu menuWithArray:buttons];
    () = msg![env; menu alignItemsVerticallyWithPadding:(size.height * 0.019)];
    () = msg![env; menu setPosition:(CGPoint { x: size.width * 0.5, y: size.height * 0.435 })];
    () = msg![env; scene addChild:menu z:20i32 tag:MENU_TAG];
    () = msg![env; director replaceScene:scene];
}

fn menu_action(env: &mut Environment, target: id, selector: SEL, sender: id) {
    let action = selector.as_str(&env.mem);
    match action {
        OPEN => show_opponents(env, target),
        CLOSE => {
            let director: id = msg_class![env; CCDirector sharedDirector];
            let tournament: id = msg_class![env; TournamentNode scene];
            () = msg![env; director replaceScene:tournament];
        }
        SELECT => {
            let index: i32 = msg![env; sender tag];
            if let Some(team) = data().teams.get(index as usize) {
                let strings = locale(env);
                if let Err(body) = start_fight(env, team, strings) {
                    let title = text(env, strings.title.as_str());
                    let message = text(env, body);
                    let close = text(env, strings.close.as_str());
                    let _: id = msg_class![env; MonsterHatchPopup showPopupWithTitle:title message:message closeButtonName:close];
                }
            }
        }
        _ => (),
    }
}

fn add_monster(env: &mut Environment, team: id, name: id, members: &mut HashSet<id>) -> bool {
    if name == nil { return false; }
    let monster: id = msg_class![env; Monster monsterOfShortType:name level:LEVEL];
    if monster == nil { return false; }
    env.mem.write(Ptr::from_bits(monster.to_bits() + TIER_IVAR), RANK_S);
    members.insert(monster);
    () = msg![env; team addObject:monster];
    true
}

fn start_fight(env: &mut Environment, opponent: &Team, strings: &'static Text) -> Result<(), &'static str> {
    let player: id = msg_class![env; Player sharedInstance];
    let roster: id = msg![env; player monstersFromBattingOrderWithAlive:false];
    let count: u32 = msg![env; roster count];
    if count == 0 { return Err(&strings.empty); }
    let mut members = HashSet::new();
    let friends: id = msg_class![env; NSMutableArray array];
    for index in 0..count {
        let original: id = msg![env; roster objectAtIndex:index];
        let name: id = msg![env; original instanceName];
        if !add_monster(env, friends, name, &mut members) { return Err(&strings.failed); }
    }
    let enemies: id = msg_class![env; NSMutableArray array];
    for name in &opponent.roster {
        let name = text(env, name.as_str());
        if !add_monster(env, enemies, name, &mut members) { return Err(&strings.failed); }
    }
    // Native itemSlotGetName maps slot 0 to License1, the Ocarina slot.
    let ocarina: id = msg![env; player gemInLicenseSlot:0u32];
    if ocarina != nil {
        () = msg![env; ocarina applyBuffsToMonsters:friends];
        () = msg![env; ocarina applyBuffsToMonsters:enemies];
    }

    let arguments: id = msg_class![env; NSMutableDictionary dictionary];
    put(env, arguments, "playerTeamOverride", friends);
    put(env, arguments, "enemyTeam", enemies);
    let background = get_static_str(env, "ArenaCombat.jpg");
    put(env, arguments, "combatBackground", background);
    let breeder: id = msg_class![env; NSMutableDictionary dictionary];
    let normal = get_static_str(env, "Normal");
    put(env, breeder, "type", normal);
    let rules: id = msg_class![env; NSMutableDictionary dictionary];
    let yes: id = msg_class![env; NSNumber numberWithBool:true];
    let no: id = msg_class![env; NSNumber numberWithBool:false];
    put(env, rules, "blocksRetreat", yes);
    put(env, rules, "allowCapture", no);
    put(env, breeder, "arguments", rules);
    put(env, arguments, "breeder", breeder);
    let encounter: id = msg_class![env; TownEncounter encounterWithArguments:arguments];
    if encounter == nil { return Err(&strings.failed); }
    () = msg![env; encounter setAllowRetreat:false];
    if env.options.capture_frames {
        let normalized = members.iter().all(|&member| {
            let level: u32 = msg![env; member level];
            let rank: u32 = msg![env; member tier];
            level == LEVEL && rank == RANK_S
        });
        log!("Additional Arena Fights level 75 and rank S verified by guest: {normalized}");
    }
    env.objc.hunter_additional_arena.encounter = Some(encounter);
    env.objc.hunter_additional_arena.members = members;
    let scene: id = msg_class![env; AwardingBattleNode sceneWithEncounter:encounter];
    let director: id = msg_class![env; CCDirector sharedDirector];
    () = msg![env; director replaceScene:scene];
    log!("Additional Arena Fights started: {} · {count} vs {} · shared ocarina={}", opponent.id, opponent.roster.len(), ocarina != nil);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_localized_ranked_rosters() {
        let arena = data();
        assert_eq!(arena.teams.len(), 10);
        assert_eq!(arena.teams[0].id, "legendary-arkwin");
        assert_eq!(arena.teams.last().unwrap().id, "bjorhyn-leaderboard");
        for team in &arena.teams {
            assert_eq!(team.roster.iter().collect::<HashSet<_>>().len(), 15);
        }
        assert_eq!(arena.locales.len(), 6);
        for key in ["de", "en", "fr", "es", "it", "ja"] {
            let text = &arena.locales[key];
            assert_eq!(text.teams.iter().collect::<HashSet<_>>().len(), 10);
            assert!(text.teams.iter().all(|name| !name.trim().is_empty()));
            assert!([&text.title, &text.choose, &text.close, &text.empty, &text.failed]
                .iter().all(|label| !label.trim().is_empty()));
        }
    }
}
