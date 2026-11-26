/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Optional local silver exchange in Hunter Island 2.02's own shop UI.
use super::{autorelease, id, msg, msg_class, nil, ClassHostObject, HostIMP, ObjC, SEL};
use crate::abi::GuestFunction;
use crate::cpu::Cpu;
use crate::environment::ThreadId;
use crate::frameworks::core_graphics::{CGPoint, CGSize};
use crate::frameworks::foundation::ns_string::from_rust_string;
use crate::Environment;

mod locale;

const CC_MENU_REGISTER_TOUCH: u32 = 0x3d5b9;
const STORE_BECAME_ACTIVE: u32 = 0x1a8b69;
const SAVE_WRITER: u32 = 0x163a2d;
const SAVE_WRITER_RETURN: u32 = 0xddea1;
const MENU_TAG: i32 = 0x4849_4558;
const RATE: i32 = 50;
const OPEN: &str = "touchHLEHunterOpenSilverExchange:";
const CANCEL: &str = "touchHLEHunterCancelSilverExchange:";
const AMOUNTS: [(i32, &str); 3] = [
    (1, "touchHLEHunterExchangeOne:"),
    (10, "touchHLEHunterExchangeTen:"),
    (100, "touchHLEHunterExchangeHundred:"),
];

pub(super) struct SaveReceipt {
    thread: ThreadId,
    written: bool,
}

pub(super) fn register_menu_touch(env: &mut Environment, menu: id, imp: GuestFunction) -> bool {
    register_tagged_menu_touch(env, menu, imp, MENU_TAG)
}

pub(super) fn register_tagged_menu_touch(env: &mut Environment, menu: id, imp: GuestFunction, expected_tag: i32) -> bool {
    if imp.addr_with_thumb_bit() != CC_MENU_REGISTER_TOUCH {
        return false;
    }
    let regs = *env.cpu.regs();
    let flags = env.cpu.cpsr();
    let tag: i32 = msg![env; menu tag];
    *env.cpu.regs_mut() = regs;
    env.cpu.set_cpsr(flags);
    if tag != expected_tag {
        return false;
    }
    let director: id = msg_class![env; CCDirector sharedDirector];
    let dispatcher: id = msg![env; director touchDispatcher];
    // Same priority as StoreNode and Scale9Popup. At equal priority the
    // newest delegate goes first. The child menu registers after its shop;
    // a modal opened afterward therefore still receives touches first.
    () = msg![env; dispatcher addTargetedDelegate:menu priority:(i32::MIN + 90)
        swallowsTouches:1i8];
    true
}

pub(super) fn after_guest_call(env: &mut Environment, receiver: id, imp: GuestFunction) {
    match imp.addr_with_thumb_bit() {
        SAVE_WRITER if env.cpu.regs()[Cpu::LR] == SAVE_WRITER_RETURN => {
            if let Some(receipt) = env.objc.hunter_exchange_save.as_mut() {
                if receipt.thread == env.current_thread {
                    receipt.written = env.cpu.regs()[0] != 0;
                }
            }
        }
        STORE_BECAME_ACTIVE => {
            if env.bundle.bundle_identifier() == "pockettrend.hunterisland"
                && env.bundle.bundle_version() == "2.02"
                && env
                    .objc
                    .try_get_class_name(ObjC::read_isa(receiver, &env.mem))
                    == Some("StoreNode")
            {
                install_menu(env, receiver);
            }
        }
        _ => (),
    }
}

fn text(env: &mut Environment, value: impl Into<String>) -> id {
    let value = from_rust_string(env, value.into());
    autorelease(env, value)
}

fn install_menu(env: &mut Environment, store: id) {
    let existing: id = msg![env; store getChildByTag:MENU_TAG];
    if existing != nil {
        return;
    }
    let class = ObjC::read_isa(store, &env.mem);
    // CCMenuItem keeps its target in a copied block. Use the class object
    // rather than the owning StoreNode to avoid a retain cycle.
    let metaclass = ObjC::read_isa(class, &env.mem);
    let open = register_action(
        env,
        metaclass,
        OPEN,
        b"v12@0:4@8",
        &(open_exchange as fn(&mut Environment, id, SEL, id)),
    );
    let select = &(select_amount as fn(&mut Environment, id, SEL, id));
    for (_, name) in AMOUNTS {
        register_action(env, class, name, b"v12@0:4@8", select);
    }
    register_action(env, class, CANCEL, b"v12@0:4@8", select);

    let director: id = msg_class![env; CCDirector sharedDirector];
    let size: CGSize = msg![env; director winSize];
    let strings = locale::preferred(env);
    let label = text(env, strings.title);
    let item: id = msg_class![env; CCMenuItemFont itemWithString:label target:class selector:open];
    () = msg![env; item setFontSize:((size.width * 0.028).round() as u32)];
    let item_size: CGSize = msg![env; item contentSize];
    let padding = size.width * 0.009;
    let brown = u32::from_le_bytes([92, 50, 21, 245]);
    let background: id = msg_class![env; CCLayerColor layerWithColor:brown
        width:(item_size.width + 2.0 * padding) height:(item_size.height + padding)];
    () = msg![env; background setPosition:(CGPoint { x: -padding, y: -padding * 0.5 })];
    () = msg![env; item addChild:background z:(-1i32)];
    let items: id = msg_class![env; NSArray arrayWithObject:item];
    let menu: id = msg_class![env; CCMenu menuWithArray:items];
    () = msg![env; menu setPosition:(CGPoint { x: size.width * 0.81, y: size.height * 0.102 })];
    () = msg![env; store addChild:menu z:20i32 tag:MENU_TAG];
}

pub(super) fn register_action(
    env: &mut Environment,
    class: id,
    name: &str,
    signature: &[u8],
    implementation: &'static dyn HostIMP,
) -> SEL {
    let selector = env
        .objc
        .register_host_selector(name.to_owned(), &mut env.mem);
    if !env.objc.class_has_uninherited_method(class, selector) {
        let types = env.mem.alloc_and_write_cstr(signature).cast_const();
        env.objc
            .add_host_instance_method_if_absent(class, selector, implementation);
        env.objc
            .borrow_mut::<ClassHostObject>(class)
            .guest_method_signatures
            .insert(selector, types);
    }
    selector
}

fn notice(env: &mut Environment, title: &str, message: impl Into<String>) {
    let title = text(env, title);
    let body = text(env, message);
    let ok = text(env, "OK");
    let _: id =
        msg_class![env; MonsterHatchPopup showPopupWithTitle:title message:body closeButtonName:ok];
}

fn open_exchange(env: &mut Environment, _class: id, _selector: SEL, item: id) {
    let menu: id = msg![env; item parent];
    let store: id = msg![env; menu parent];
    if store == nil {
        return;
    }
    let save: id = msg_class![env; UserSavedData sharedInstance];
    let silver: i32 = msg![env; save silverAvailable];
    let strings = locale::preferred(env);
    if silver < RATE {
        notice(
            env,
            strings.title,
            strings.minimum.replace("{rate}", &RATE.to_string()),
        );
        return;
    }

    let title = text(env, strings.title);
    let rate = strings.rate.replace("{rate}", &RATE.to_string());
    let balance = strings.balance.replace("{silver}", &silver.to_string());
    let message = text(env, format!("{rate}\n{balance}"));
    // MonsterHatchPopup supplies the concrete popup skin and button layout.
    let layout: id = msg_class![env; MonsterHatchPopup popupTextConfiguration];
    let content: id = msg![env; layout generateContentWithMessage:message];
    let config: id = msg_class![env; MonsterHatchPopup popupConfigurationForContent:content];
    () = msg![env; config setTitle:title];
    let buttons: id = msg![env; config buttonConfigurations];
    for (gold, name) in AMOUNTS {
        if gold * RATE <= silver {
            let amount = strings
                .amount
                .replace("{gold}", &gold.to_string())
                .replace("{cost}", &(gold * RATE).to_string());
            let title = text(env, amount);
            add_popup_button(env, buttons, store, title, name);
        }
    }
    let cancel = text(env, strings.cancel);
    add_popup_button(env, buttons, store, cancel, CANCEL);
    let _: id = msg_class![env; MonsterHatchPopup showPopupWithConfiguration:config];
}

fn add_popup_button(env: &mut Environment, buttons: id, store: id, name: id, action: &str) {
    let selector = env.objc.lookup_selector(action).unwrap();
    let button: id = msg_class![env; MonsterHatchPopup popupButtonConfigurationForName:name
        target:store selector:selector];
    () = msg![env; buttons addObject:button];
}

fn select_amount(env: &mut Environment, store: id, selector: SEL, _button: id) {
    let Some((amount, _)) = AMOUNTS
        .into_iter()
        .find(|(_, name)| env.objc.lookup_selector(name) == Some(selector))
    else {
        return;
    };
    let error = match exchange(env, store, amount) {
        Ok(()) => return,
        Err(error) => error,
    };
    let strings = locale::preferred(env);
    let message = match error {
        ExchangeError::Unavailable => strings.unavailable,
        ExchangeError::Insufficient => strings.insufficient,
        ExchangeError::SaveFailed => strings.save_failed,
    };
    notice(env, strings.title, message);
}

enum ExchangeError {
    Unavailable,
    Insufficient,
    SaveFailed,
}

fn exchange(env: &mut Environment, store: id, amount: i32) -> Result<(), ExchangeError> {
    let save: id = msg_class![env; UserSavedData sharedInstance];
    if save == nil || env.objc.hunter_exchange_save.is_some() {
        return Err(ExchangeError::Unavailable);
    }
    let ivar = |name: &str| {
        env.objc
            .object_lookup_ivar(&env.mem, save, &name.to_owned())
            .ok_or(ExchangeError::Unavailable)
    };
    let silver = ivar("silver")?.cast::<i32>();
    let gold = ivar("goldFound")?.cast::<i32>();
    let dirty = ivar("saveFileDirty")?.cast::<u8>();
    let mangle = env.mem.read(ivar("mangle")?.cast::<i32>());
    let purchased = env.mem.read(ivar("goldPurchased")?.cast::<i32>());
    let spent = env.mem.read(ivar("goldSpent")?.cast::<i32>());
    let old_silver = env.mem.read(silver);
    let old_gold = env.mem.read(gold);
    let old_dirty = env.mem.read(dirty);
    let (next_silver, next_gold) =
        updated_balances(old_silver, old_gold, purchased, spent, mangle, amount)
            .ok_or(ExchangeError::Insufficient)?;

    // SpendSilver: saves immediately, and addFoundGold: applies the save's
    // encoding multiplier. Change the two stored balances together, then ask
    // the game's normal atomic writer to commit the whole save once.
    env.mem.write(silver, next_silver);
    env.mem.write(gold, next_gold);
    env.mem.write(dirty, 1);
    let history_before =
        super::hunter_history::exchange_snapshot(env, (amount * RATE) as u64, amount as u64);
    env.objc.hunter_exchange_save = Some(SaveReceipt {
        thread: env.current_thread,
        written: false,
    });
    () = msg![env; save safeSaveToFile];
    if !env.objc.hunter_exchange_save.take().unwrap().written {
        env.mem.write(silver, old_silver);
        env.mem.write(gold, old_gold);
        env.mem.write(dirty, old_dirty);
        super::hunter_history::exchange_rollback(env, history_before);
        return Err(ExchangeError::SaveFailed);
    }
    () = msg![env; store updateAll];
    log!("Hunter Island silver exchange: silver {old_silver} -> {next_silver}, gold +{amount}, saved");
    Ok(())
}

fn updated_balances(
    silver: i32,
    found: i32,
    purchased: i32,
    spent: i32,
    mangle: i32,
    amount: i32,
) -> Option<(i32, i32)> {
    if amount <= 0 || mangle <= 0 {
        return None;
    }
    let cost = amount.checked_mul(RATE)?;
    let new_silver = silver.checked_sub(cost)?;
    if new_silver < 0 {
        return None;
    }
    let added = amount.checked_mul(mangle)?;
    let new_found = found.checked_add(added)?;
    let available = new_found.checked_add(purchased)?.checked_sub(spent)?;
    // goldAvailable clamps negative balances to zero. Never charge silver if
    // that would make the visible gold gain smaller than the promised amount.
    if available < added {
        return None;
    }
    Some((new_silver, new_found))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exchange_checks_cost_save_encoding_and_integer_bounds() {
        assert_eq!(updated_balances(50, 10, 7, 3, 2, 1), Some((0, 12)));
        assert_eq!(updated_balances(501, 30, 20, 5, 3, 10), Some((1, 60)));
        assert_eq!(updated_balances(49, 10, 0, 0, 1, 1), None);
        assert_eq!(updated_balances(50, 10, 0, 12, 1, 1), None);
        assert_eq!(updated_balances(50, 10, 0, 0, 0, 1), None);
        assert_eq!(updated_balances(50, i32::MAX - 1, 1, 0, 1, 1), None);
    }
}
