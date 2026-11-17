/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use crate::frameworks::foundation::ns_string::to_rust_string;
use crate::objc::{id, msg, msg_class};
use crate::Environment;

pub(super) struct ExchangeText {
    pub(super) title: &'static str,
    pub(super) rate: &'static str,
    pub(super) balance: &'static str,
    pub(super) amount: &'static str,
    pub(super) cancel: &'static str,
    pub(super) minimum: &'static str,
    pub(super) unavailable: &'static str,
    pub(super) insufficient: &'static str,
    pub(super) save_failed: &'static str,
}

pub(super) fn preferred(env: &mut Environment) -> ExchangeText {
    let languages: id = msg_class![env; NSLocale preferredLanguages];
    let count: u32 = msg![env; languages count];
    for index in 0..count {
        let language: id = msg![env; languages objectAtIndex:index];
        if let Some(strings) = for_language(&to_rust_string(env, language)) {
            return strings;
        }
    }
    for_language("en").unwrap()
}

fn for_language(tag: &str) -> Option<ExchangeText> {
    let language = tag.split(['-', '_']).next()?.to_ascii_lowercase();
    Some(match language.as_str() {
        "en" => ExchangeText {
            title: "Exchange silver",
            rate: "{rate} silver = 1 gold",
            balance: "Available: {silver} silver",
            amount: "{gold} gold ({cost})",
            cancel: "Cancel",
            minimum: "You need at least {rate} silver for 1 gold.",
            unavailable: "Exchange is currently unavailable for this save.",
            insufficient: "You don't have enough silver for this exchange.",
            save_failed: "Saving failed. The exchange was reversed.",
        },
        "de" => ExchangeText {
            title: "Silber tauschen",
            rate: "{rate} Silber = 1 Gold",
            balance: "Verfügbar: {silver} Silber",
            amount: "{gold} Gold ({cost})",
            cancel: "Abbrechen",
            minimum: "Du brauchst mindestens {rate} Silber für 1 Gold.",
            unavailable: "Der Tausch ist in diesem Spielstand gerade nicht verfügbar.",
            insufficient: "Das Guthaben reicht für diesen Tausch nicht aus.",
            save_failed: "Speichern fehlgeschlagen. Der Tausch wurde rückgängig gemacht.",
        },
        "es" => ExchangeText {
            title: "Cambiar plata",
            rate: "{rate} de plata = 1 de oro",
            balance: "Disponible: {silver} de plata",
            amount: "{gold} oro ({cost})",
            cancel: "Cancelar",
            minimum: "Necesitas al menos {rate} de plata para obtener 1 de oro.",
            unavailable: "El cambio no está disponible en esta partida.",
            insufficient: "No tienes suficiente plata para este cambio.",
            save_failed: "No se pudo guardar. Se anuló el cambio.",
        },
        "fr" => ExchangeText {
            title: "Échanger l’argent",
            rate: "{rate} argent = 1 or",
            balance: "Solde : {silver} argent",
            amount: "{gold} or ({cost})",
            cancel: "Annuler",
            minimum: "Il faut au moins {rate} pièces d’argent pour 1 pièce d’or.",
            unavailable: "L’échange est indisponible pour cette sauvegarde.",
            insufficient: "Tu n’as pas assez de pièces d’argent pour cet échange.",
            save_failed: "Échec de la sauvegarde. L’échange a été annulé.",
        },
        "it" => ExchangeText {
            title: "Scambia argento",
            rate: "{rate} argento = 1 oro",
            balance: "Disponibile: {silver} argento",
            amount: "{gold} oro ({cost})",
            cancel: "Annulla",
            minimum: "Servono almeno {rate} monete d’argento per 1 moneta d’oro.",
            unavailable: "Lo scambio non è disponibile in questo salvataggio.",
            insufficient: "Non hai abbastanza argento per questo scambio.",
            save_failed: "Salvataggio fallito. Lo scambio è stato annullato.",
        },
        "ja" => ExchangeText {
            title: "銀を交換",
            rate: "銀{rate}枚 = 金1枚",
            balance: "所持：銀{silver}枚",
            amount: "金{gold}（銀{cost}）",
            cancel: "キャンセル",
            minimum: "金1枚との交換には銀が{rate}枚以上必要です。",
            unavailable: "このセーブデータでは現在交換できません。",
            insufficient: "交換に必要な銀が足りません。",
            save_failed: "セーブに失敗しました。交換は取り消されました。",
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::for_language;

    #[test]
    fn exchange_supports_every_game_locale_with_complete_amount_templates() {
        for code in ["en", "de-DE", "es_MX", "fr-CA", "it", "JA-JP"] {
            let strings = for_language(code).unwrap();
            assert!(strings.rate.contains("{rate}"));
            assert!(strings.balance.contains("{silver}"));
            assert!(strings.amount.contains("{gold}"));
            assert!(strings.amount.contains("{cost}"));
            assert!(strings.minimum.contains("{rate}"));
        }
        assert!(for_language("pt-BR").is_none());
    }
}
