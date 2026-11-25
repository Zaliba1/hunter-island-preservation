// SPDX-License-Identifier: MPL-2.0
//! Optional additions to Orlen's existing question and native loot events.
use plist::{Dictionary, Value};
use std::collections::BTreeMap;

pub(crate) const JOIN: &str = "DestroyOnPickup.GravestoneJoin";
pub(crate) const TOTEM: &str = "DestroyOnPickup.GravestoneTotem";
pub(crate) const PROMPTS: [(&str, &str); 6] = [
    (
        "en",
        "You can also enter a number from the Monsterpedia (1-360).",
    ),
    (
        "de",
        "Du kannst auch eine Nummer aus der Monsterpedia eingeben (1-360).",
    ),
    (
        "fr",
        "Tu peux aussi saisir le numéro d’une créature de l’encyclopédie (1 à 360).",
    ),
    (
        "es",
        "También puedes introducir el número de una criatura de la enciclopedia (1-360).",
    ),
    (
        "it",
        "Puoi anche inserire il numero di una creatura dell’enciclopedia (1-360).",
    ),
    ("ja", "図鑑番号（1から360）を入力することもできます。"),
];
fn dict<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Dictionary(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
    )
}

pub(crate) fn items(items: &mut Value) -> Result<(), &'static str> {
    let types = items
        .as_dictionary_mut()
        .and_then(|d| d.get_mut("DestroyOnPickup"))
        .and_then(Value::as_dictionary_mut)
        .ok_or("Missing pickup item types")?;
    for (original, alias) in [
        ("MonsterJoin", "GravestoneJoin"),
        ("SpiritTotem", "GravestoneTotem"),
    ] {
        let template = types
            .get(original)
            .ok_or("Missing reward template")?
            .clone();
        if types.insert(alias.into(), template).is_some() {
            return Err("Duplicate gravestone item");
        }
    }
    Ok(())
}

fn reward(short: &str, summon: bool) -> Value {
    let mut args = Dictionary::new();
    args.insert("shortType".into(), short.into());
    if !summon {
        args.insert("jobLevel".into(), 1.into());
    }
    let item = dict([
        ("type", if summon { TOTEM } else { JOIN }.into()),
        ("arguments", args.into()),
    ]);
    dict([
        ("class", "GiveItemsGameFlowEvent".into()),
        (
            "arguments",
            dict([
                ("forceNotUnique", true.into()),
                (
                    "treasureData",
                    dict([
                        ("levelScalor", 1.into()),
                        (
                            "items",
                            Value::Array(vec![dict([
                                ("spawnType", "fixed".into()),
                                ("data", item),
                            ])]),
                        ),
                    ]),
                ),
                (
                    "nextEvent",
                    dict([
                        ("class", "MoveToTownGameFlowEvent".into()),
                        ("arguments", Value::Dictionary(Dictionary::new())),
                    ]),
                ),
            ]),
        ),
    ])
}

pub(crate) fn towns(towns: &mut Value, monsters: &Value) -> Result<(), &'static str> {
    let mut book = BTreeMap::new();
    for (group, entries) in monsters.as_dictionary().ok_or("Invalid monster catalog")? {
        if group == "RootMonster" {
            continue;
        }
        for (short, template) in entries.as_dictionary().ok_or("Invalid monster group")? {
            let entry = template
                .as_dictionary()
                .ok_or("Invalid monster definition")?;
            let Some(index) = entry.get("index") else {
                continue;
            };
            let order = index
                .as_signed_integer()
                .or_else(|| {
                    index
                        .as_real()
                        .filter(|v| v.is_finite() && v.fract() == 0.0)
                        .map(|v| v as i64)
                })
                .ok_or("Invalid book index")?;
            let summon = match entry.get("category").and_then(Value::as_string) {
                Some("Summon") => true,
                Some("Normal" | "Combine" | "Egg") => false,
                _ => return Err("Unknown monster category"),
            };
            if book.insert(order, (short, summon)).is_some() {
                return Err("Ambiguous book order");
            }
        }
    }
    if book.len() != 360 {
        return Err("Unsupported Monsterpedia");
    }
    let buildings = towns
        .as_dictionary_mut()
        .and_then(|d| d.get_mut("over-26x111"))
        .and_then(Value::as_dictionary_mut)
        .and_then(|d| d.get_mut("buildings"))
        .and_then(Value::as_array_mut)
        .ok_or("Missing Orlen town menu")?;
    let stone = buildings
        .iter_mut()
        .filter_map(Value::as_dictionary_mut)
        .find(|b| {
            b.get("arguments")
                .and_then(Value::as_dictionary)
                .and_then(|a| a.get("promptEvent"))
                .and_then(Value::as_string)
                == Some("chat:DI.DIQuestion")
        })
        .ok_or("Missing Orlen gravestone")?;
    let answers = stone
        .get_mut("arguments")
        .and_then(Value::as_dictionary_mut)
        .and_then(|a| a.get_mut("answerEvents"))
        .and_then(Value::as_dictionary_mut)
        .ok_or("Missing gravestone answers")?;
    for (i, (short, summon)) in book.into_values().enumerate() {
        let number = i + 1;
        let event = reward(short, summon);
        if number < 100 {
            answers.insert(format!("{number:03}"), event.clone());
        }
        answers.insert(number.to_string(), event);
    }
    // Retail hides the stone after the correct story answer. Keep two disjoint
    // states so that answer and its unique reward still work exactly once.
    let mut solved = stone.clone();
    solved.insert(
        "conditions".into(),
        dict([("chats", "DI.DIRightAnswer".into())]),
    );
    solved
        .get_mut("arguments")
        .and_then(Value::as_dictionary_mut)
        .and_then(|a| a.get_mut("answerEvents"))
        .and_then(Value::as_dictionary_mut)
        .and_then(|a| a.remove("DIQuestionAnswer"))
        .ok_or("Missing retail answer")?;
    buildings.push(solved.into());
    Ok(())
}
