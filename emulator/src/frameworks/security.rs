/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! App-local generic-password storage for preservation. This is NOT a secure
//! system keychain: data is a plaintext binary plist in this app's sandbox.
//! No host credentials, cross-app access groups, device locking or encryption.
//! Unsupported classes/options return errors rather than reporting success.
//! Constant values: apple-oss-distributions/Security, SecItemConstants.c.
use crate::dyld::{ConstantExports, FunctionExports, HostConstant, HostDylib};
use crate::export_c_func;
use crate::frameworks::foundation::{ns_data, ns_dictionary, ns_string};
use crate::fs::Fs;
use crate::mem::{MutPtr, MutVoidPtr};
use crate::objc::{id, msg, msg_class, nil, release, Class};
use crate::Environment;
use plist::{Dictionary, Value};
use std::io::Cursor;

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/Security.framework/Security",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[CONSTANTS],
    function_exports: &[FUNCTIONS],
};

type OSStatus = i32;
const SUCCESS: OSStatus = 0;
const PARAM: OSStatus = -50;
const UNIMPLEMENTED: OSStatus = -4;
const IO: OSStatus = -36;
const DUPLICATE: OSStatus = -25299;
const NOT_FOUND: OSStatus = -25300;
const DECODE: OSStatus = -26275;
const STORE_NAME: &str = "touchHLE-keychain.plist";

#[derive(Clone, Copy, PartialEq)]
enum Operation {
    Add,
    Copy,
    Update,
    Delete,
}

fn is_kind(env: &mut Environment, object: id, name: &str) -> bool {
    if object == nil {
        return false;
    }
    let class: Class = msg![env; object class];
    let expected = env.objc.get_known_class(name, &mut env.mem);
    env.objc.class_is_subclass_of(class, expected)
}

/// Copy supported values into owned host memory. Never persist guest pointers.
fn dictionary(env: &mut Environment, object: id) -> Result<Dictionary, OSStatus> {
    if !is_kind(env, object, "NSDictionary") {
        return Err(PARAM);
    }
    let keys: id = msg![env; object allKeys];
    let count: u32 = msg![env; keys count];
    if count > 64 {
        return Err(PARAM);
    }
    let mut result = Dictionary::new();
    for index in 0..count {
        let key: id = msg![env; keys objectAtIndex:index];
        if !is_kind(env, key, "NSString") {
            return Err(PARAM);
        }
        let name = ns_string::to_rust_string(env, key).into_owned();
        let value: id = msg![env; object objectForKey:key];
        let value = if is_kind(env, value, "NSString") {
            Value::String(ns_string::to_rust_string(env, value).into_owned())
        } else if is_kind(env, value, "NSData") {
            let data = ns_data::to_rust_slice(env, value);
            if data.len() > 16 * 1024 * 1024 {
                return Err(PARAM);
            }
            Value::Data(data.to_vec())
        } else if is_kind(env, value, "NSNumber") {
            let boolean: bool = msg![env; value boolValue];
            Value::Boolean(boolean)
        } else {
            return Err(PARAM);
        };
        result.insert(name, value);
    }
    Ok(result)
}

fn validate(values: &Dictionary, op: Operation, changes: bool) -> Result<(), OSStatus> {
    if !changes {
        match values.get("class").and_then(Value::as_string) {
            Some("genp") => (),
            Some(_) => return Err(UNIMPLEMENTED),
            None => return Err(PARAM),
        }
    }
    for (key, value) in values {
        let valid = match key.as_str() {
            "class" => !changes && value.as_string() == Some("genp"),
            "acct" | "svce" | "desc" | "labl" => value.as_string().is_some(),
            "pdmn" => matches!(value.as_string(), Some("ak" | "ck" | "dk")),
            "gena" | "v_Data" => value.as_data().is_some(),
            "r_Data" | "r_Attributes"
                if !changes && matches!(op, Operation::Add | Operation::Copy) =>
            {
                value.as_boolean().is_some()
            }
            "m_Limit" if !changes && op == Operation::Copy => {
                value.as_string() == Some("m_LimitOne")
            }
            _ => return Err(UNIMPLEMENTED),
        };
        if !valid {
            return Err(PARAM);
        }
    }
    Ok(())
}

fn attributes(values: &Dictionary) -> Dictionary {
    values
        .iter()
        .filter(|(k, _)| !matches!(k.as_str(), "r_Data" | "r_Attributes" | "m_Limit"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

fn identity(item: &Dictionary) -> (&str, &str) {
    // Absent account/service have the same primary-key value as empty strings.
    (
        item.get("acct").and_then(Value::as_string).unwrap_or(""),
        item.get("svce").and_then(Value::as_string).unwrap_or(""),
    )
}

fn matches(item: &Dictionary, query: &Dictionary) -> bool {
    attributes(query).iter().all(|(key, value)| {
        if matches!(key.as_str(), "acct" | "svce") {
            item.get(key).and_then(Value::as_string).unwrap_or("") == value.as_string().unwrap()
        } else {
            item.get(key) == Some(value)
        }
    })
}

fn load(fs: &Fs) -> Result<Vec<Dictionary>, OSStatus> {
    let path = fs
        .home_directory()
        .join("Library")
        .join("Preferences")
        .join(STORE_NAME);
    if !fs.exists(&path) {
        return Ok(Vec::new());
    }
    let bytes = fs.read(&path).map_err(|_| IO)?;
    let root = Value::from_reader(Cursor::new(bytes)).map_err(|_| DECODE)?;
    let root = root.as_dictionary().ok_or(DECODE)?;
    if root.get("version").and_then(Value::as_unsigned_integer) != Some(1) {
        return Err(DECODE);
    }
    let items = root.get("items").and_then(Value::as_array).ok_or(DECODE)?;
    let mut result = Vec::new();
    for item in items {
        let item = item.as_dictionary().ok_or(DECODE)?;
        validate(item, Operation::Add, false).map_err(|_| DECODE)?;
        if attributes(item) != *item || result.iter().any(|other| identity(other) == identity(item))
        {
            return Err(DECODE);
        }
        result.push(item.clone());
    }
    Ok(result)
}

fn persist(fs: &mut Fs, items: &[Dictionary]) -> Result<(), OSStatus> {
    let mut root = Dictionary::new();
    root.insert("version".into(), Value::from(1u64));
    root.insert(
        "items".into(),
        Value::Array(items.iter().cloned().map(Value::Dictionary).collect()),
    );
    let mut bytes = Vec::new();
    Value::Dictionary(root)
        .to_writer_binary(&mut bytes)
        .map_err(|_| IO)?;
    let path = fs
        .home_directory()
        .join("Library")
        .join("Preferences")
        .join(STORE_NAME);
    fs.write_atomic(&path, &bytes).map_err(|_| IO)
}

/// Modify a private snapshot; the caller commits it only after all checks pass.
fn modify(
    items: &mut Vec<Dictionary>,
    query: &Dictionary,
    changes: &Dictionary,
    op: Operation,
) -> Result<Dictionary, OSStatus> {
    if op == Operation::Add {
        let mut item = attributes(query);
        for key in ["acct", "svce"] {
            if !item.contains_key(key) {
                item.insert(key.into(), Value::String(String::new()));
            }
        }
        if !item.contains_key("v_Data") {
            item.insert("v_Data".into(), Value::Data(Vec::new()));
        }
        if !item.contains_key("pdmn") {
            item.insert("pdmn".into(), Value::String("ak".into()));
        }
        if items.iter().any(|other| identity(other) == identity(&item)) {
            return Err(DUPLICATE);
        }
        items.push(item.clone());
        return Ok(item);
    }
    let selected: Vec<_> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| matches(item, query))
        .map(|(i, _)| i)
        .collect();
    let first = *selected.first().ok_or(NOT_FOUND)?;
    if op == Operation::Copy {
        return Ok(items[first].clone());
    }
    if op == Operation::Delete {
        for index in selected.into_iter().rev() {
            items.remove(index);
        }
        return Ok(Dictionary::new());
    }
    for index in selected {
        for (key, value) in changes {
            items[index].insert(key.clone(), value.clone());
        }
    }
    for (index, item) in items.iter().enumerate() {
        if items[..index]
            .iter()
            .any(|other| identity(other) == identity(item))
        {
            return Err(DUPLICATE);
        }
    }
    Ok(Dictionary::new())
}

fn data_object(env: &mut Environment, bytes: &[u8]) -> id {
    let length: u32 = bytes.len().try_into().unwrap();
    let buffer: MutVoidPtr = env.mem.alloc(length);
    env.mem
        .bytes_at_mut(buffer.cast(), length)
        .copy_from_slice(bytes);
    let data: id = msg_class![env; NSData alloc];
    msg![env; data initWithBytesNoCopy:buffer length:length]
}

fn result_object(env: &mut Environment, item: &Dictionary, query: &Dictionary) -> id {
    let data = query
        .get("r_Data")
        .and_then(Value::as_boolean)
        .unwrap_or(false);
    let attrs = query
        .get("r_Attributes")
        .and_then(Value::as_boolean)
        .unwrap_or(false);
    if !attrs {
        return if data {
            data_object(
                env,
                item.get("v_Data").and_then(Value::as_data).unwrap_or(&[]),
            )
        } else {
            nil
        };
    }
    let mut pairs = Vec::new();
    for (key, value) in item {
        if key == "v_Data" && !data {
            continue;
        }
        let key = ns_string::from_rust_string(env, key.clone());
        let value = match value {
            Value::String(s) => ns_string::from_rust_string(env, s.clone()),
            Value::Data(d) => data_object(env, d),
            _ => unreachable!(),
        };
        pairs.push((key, value));
    }
    let result = ns_dictionary::dict_from_keys_and_objects(env, &pairs);
    for (key, value) in pairs {
        release(env, key);
        release(env, value);
    }
    result
}

fn execute(
    env: &mut Environment,
    query: id,
    changes: id,
    result: MutPtr<id>,
    op: Operation,
) -> OSStatus {
    if !result.is_null() {
        env.mem.write(result, nil);
    }
    let attempt = (|| {
        let query = dictionary(env, query)?;
        validate(&query, op, false)?;
        let changes = if op == Operation::Update {
            let changes = dictionary(env, changes)?;
            validate(&changes, op, true)?;
            changes
        } else {
            Dictionary::new()
        };
        let mut items = load(&env.fs)?;
        let item = modify(&mut items, &query, &changes, op)?;
        if op != Operation::Copy {
            persist(&mut env.fs, &items)?;
        }
        if !result.is_null() {
            let object = result_object(env, &item, &query);
            env.mem.write(result, object);
        }
        Ok(())
    })();
    let status = attempt.err().unwrap_or(SUCCESS);
    log!(
        "App-local keychain {}: status {}",
        match op {
            Operation::Add => "add",
            Operation::Copy => "copy",
            Operation::Update => "update",
            Operation::Delete => "delete",
        },
        status
    );
    status
}

fn SecItemCopyMatching(env: &mut Environment, query: id, result: MutPtr<id>) -> OSStatus {
    execute(env, query, nil, result, Operation::Copy)
}
fn SecItemAdd(env: &mut Environment, query: id, result: MutPtr<id>) -> OSStatus {
    execute(env, query, nil, result, Operation::Add)
}
fn SecItemUpdate(env: &mut Environment, query: id, changes: id) -> OSStatus {
    execute(env, query, changes, MutPtr::null(), Operation::Update)
}
fn SecItemDelete(env: &mut Environment, query: id) -> OSStatus {
    execute(env, query, nil, MutPtr::null(), Operation::Delete)
}
const FUNCTIONS: FunctionExports = &[
    export_c_func!(SecItemCopyMatching(_, _)),
    export_c_func!(SecItemAdd(_, _)),
    export_c_func!(SecItemUpdate(_, _)),
    export_c_func!(SecItemDelete(_)),
];

fn probe_query(env: &mut Environment, data: Option<&[u8]>, returns: bool) -> id {
    let mut values = vec![
        ("class", Value::String("genp".into())),
        ("acct", Value::String("probe-account".into())),
        ("svce", Value::String("org.touchhle.keychain-probe".into())),
    ];
    if let Some(data) = data {
        values.push(("v_Data", Value::Data(data.to_vec())));
    }
    if returns {
        values.push(("r_Data", Value::Boolean(true)));
    }
    let pairs: Vec<_> = values
        .into_iter()
        .map(|(key, value)| {
            let key = ns_string::from_rust_string(env, key.into());
            let object = match value {
                Value::String(s) => ns_string::from_rust_string(env, s),
                Value::Data(d) => data_object(env, &d),
                Value::Boolean(b) => {
                    let number: id = msg_class![env; NSNumber alloc];
                    msg![env; number initWithBool:b]
                }
                _ => unreachable!(),
            };
            (key, object)
        })
        .collect();
    let query = ns_dictionary::dict_from_keys_and_objects(env, &pairs);
    for (key, object) in pairs {
        release(env, key);
        release(env, object);
    }
    query
}

fn check_copy(env: &mut Environment, query: id, expected: &[u8]) {
    let out: MutPtr<id> = env.mem.alloc(4).cast();
    assert_eq!(SecItemCopyMatching(env, query, out), SUCCESS);
    let object = env.mem.read(out);
    assert!(is_kind(env, object, "NSData"));
    assert_eq!(env.objc.get_refcount(object).get(), 1);
    assert_eq!(ns_data::to_rust_slice(env, object), expected);
    release(env, object);
    env.mem.free(out.cast());
}

fn exercise(env: &mut Environment, write: bool) {
    let pool: id = msg_class![env; NSAutoreleasePool new];
    let query = probe_query(env, None, true);
    let selector = probe_query(env, None, false);
    if write {
        assert!(matches!(SecItemDelete(env, selector), SUCCESS | NOT_FOUND));
        let add = probe_query(env, Some(b"original\0binary\xff"), false);
        assert_eq!(SecItemAdd(env, add, MutPtr::null()), SUCCESS);
        assert_eq!(SecItemAdd(env, add, MutPtr::null()), DUPLICATE);
        check_copy(env, query, b"original\0binary\xff");
        // Update dictionaries do not contain kSecClass or search controls.
        let key = ns_string::get_static_str(env, "v_Data");
        let data = data_object(env, b"updated\0binary\xfe");
        let changes = ns_dictionary::dict_from_keys_and_objects(env, &[(key, data)]);
        release(env, data);
        assert_eq!(SecItemUpdate(env, selector, changes), SUCCESS);
        release(env, changes);
        check_copy(env, query, b"updated\0binary\xfe");
        release(env, add);
    } else {
        check_copy(env, query, b"updated\0binary\xfe");
        // Request attributes and data together, checking that the result is a
        // retained dictionary, and attributes-only does not expose the data.
        let attr_key = ns_string::get_static_str(env, "r_Attributes");
        let attrs: id = msg![env; query mutableCopy];
        let boolean: id = msg_class![env; NSNumber alloc];
        let boolean: id = msg![env; boolean initWithBool:true];
        let (): () = msg![env; attrs setObject:boolean forKey:attr_key];
        release(env, boolean);
        let out: MutPtr<id> = env.mem.alloc(4).cast();
        assert_eq!(SecItemCopyMatching(env, attrs, out), SUCCESS);
        let object = env.mem.read(out);
        assert_eq!(env.objc.get_refcount(object).get(), 1);
        assert_eq!(
            dictionary(env, object)
                .unwrap()
                .get("v_Data")
                .and_then(Value::as_data),
            Some(b"updated\0binary\xfe".as_slice())
        );
        release(env, object);
        let data_key = ns_string::get_static_str(env, "r_Data");
        let (): () = msg![env; attrs removeObjectForKey:data_key];
        assert_eq!(SecItemCopyMatching(env, attrs, out), SUCCESS);
        let object = env.mem.read(out);
        assert!(!dictionary(env, object).unwrap().contains_key("v_Data"));
        release(env, object);
        release(env, attrs);
        assert_eq!(SecItemDelete(env, selector), SUCCESS);
        assert_eq!(SecItemCopyMatching(env, query, out), NOT_FOUND);
        assert_eq!(env.mem.read(out), nil);
        assert_eq!(SecItemUpdate(env, selector, selector), PARAM);
        assert_eq!(SecItemAdd(env, nil, out), PARAM);
        env.mem.free(out.cast());
    }
    release(env, query);
    release(env, selector);
    release(env, pool);
}

/// Two separate invocations demonstrate persistence through process exit.
/// Uses a dedicated fixture identity, never the game's storage or executable.
pub fn persistence_probe(write: bool) -> Result<(), String> {
    let mut environment = Environment::new_without_app(
        Default::default(),
        crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
    )?;
    let bundle_path = std::path::Path::new("keychain-probe.app");
    std::fs::create_dir_all(bundle_path).map_err(|e| e.to_string())?;
    *environment.fs = Fs::new(
        crate::fs::BundleData::HostDirectory(bundle_path.to_owned()),
        "KeychainProbe.app".into(),
        "org.touchhle.keychain-probe.v1",
        false,
    )
    .0;
    exercise(&mut environment, write);
    echo!(
        "KEYCHAIN_{}_PASS",
        if write { "WRITE" } else { "REOPEN_DELETE" }
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    fn fixture() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/keychain-tests")
            .join(format!(
                "{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir_all(path.join("Library/Preferences")).unwrap();
        path
    }
    fn item(account: &str, service: &str) -> Dictionary {
        [
            ("class", Value::String("genp".into())),
            ("acct", Value::String(account.into())),
            ("svce", Value::String(service.into())),
            ("v_Data", Value::Data(vec![0, 255])),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
    }
    #[test]
    fn keychain_persistence_errors_and_identity() {
        let path = fixture();
        let mut fs = Fs::from_test_directory(&path, true);
        assert!(load(&fs).unwrap().is_empty());
        let mut items = vec![];
        let query = item("a", "s");
        validate(&query, Operation::Add, false).unwrap();
        modify(&mut items, &query, &Dictionary::new(), Operation::Add).unwrap();
        assert_eq!(
            modify(&mut items, &query, &Dictionary::new(), Operation::Add),
            Err(DUPLICATE)
        );
        let mut metadata_variant = query.clone();
        metadata_variant.insert("gena".into(), Value::Data(vec![1]));
        assert_eq!(
            modify(
                &mut items,
                &metadata_variant,
                &Dictionary::new(),
                Operation::Add
            ),
            Err(DUPLICATE)
        );
        modify(
            &mut items,
            &item("b", "s"),
            &Dictionary::new(),
            Operation::Add,
        )
        .unwrap();
        persist(&mut fs, &items).unwrap();
        let before = std::fs::read(path.join("Library/Preferences").join(STORE_NAME)).unwrap();
        let readonly = &mut Fs::from_test_directory(&path, false);
        assert_eq!(persist(readonly, &[]), Err(IO));
        assert_eq!(
            std::fs::read(path.join("Library/Preferences").join(STORE_NAME)).unwrap(),
            before
        );
        let mut reopened = Fs::from_test_directory(&path, true);
        assert_eq!(load(&reopened).unwrap(), items);
        let changes = [("acct", Value::String("b".into()))].into_iter().collect();
        let selector = [
            ("class", Value::String("genp".into())),
            ("acct", Value::String("a".into())),
        ]
        .into_iter()
        .collect();
        let mut snapshot = items.clone();
        assert_eq!(
            modify(&mut snapshot, &selector, &changes, Operation::Update),
            Err(DUPLICATE)
        );
        assert_eq!(load(&reopened).unwrap(), items); // rejected update was not committed
        let missing = item("missing", "s");
        assert_eq!(
            modify(&mut items, &missing, &Dictionary::new(), Operation::Delete),
            Err(NOT_FOUND)
        );
        let file = reopened
            .home_directory()
            .join("Library/Preferences")
            .join(STORE_NAME);
        reopened.write_atomic(&file, b"invalid plist").unwrap();
        assert_eq!(load(&reopened), Err(DECODE));
        let mut unsupported = query;
        unsupported.insert("class".into(), Value::String("inet".into()));
        assert_eq!(
            validate(&unsupported, Operation::Add, false),
            Err(UNIMPLEMENTED)
        );
    }
    #[test]
    fn keychain_guest_api_roundtrip() {
        let path = fixture();
        let mut env = Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        *env.fs = Fs::from_test_directory(&path, true);
        exercise(&mut env, true);
        *env.fs = Fs::from_test_directory(&path, true); // discard all VFS state and reopen
        exercise(&mut env, false);
    }
}

const CONSTANTS: ConstantExports = &[
    ("_kSecClass", HostConstant::NSString("class")),
    ("_kSecClassGenericPassword", HostConstant::NSString("genp")),
    ("_kSecAttrAccessible", HostConstant::NSString("pdmn")),
    (
        "_kSecAttrAccessibleWhenUnlocked",
        HostConstant::NSString("ak"),
    ),
    (
        "_kSecAttrAccessibleAfterFirstUnlock",
        HostConstant::NSString("ck"),
    ),
    ("_kSecAttrAccessibleAlways", HostConstant::NSString("dk")),
    ("_kSecAttrAccount", HostConstant::NSString("acct")),
    ("_kSecAttrService", HostConstant::NSString("svce")),
    ("_kSecAttrGeneric", HostConstant::NSString("gena")),
    ("_kSecAttrDescription", HostConstant::NSString("desc")),
    ("_kSecAttrLabel", HostConstant::NSString("labl")),
    ("_kSecMatchLimit", HostConstant::NSString("m_Limit")),
    ("_kSecMatchLimitOne", HostConstant::NSString("m_LimitOne")),
    ("_kSecReturnData", HostConstant::NSString("r_Data")),
    (
        "_kSecReturnAttributes",
        HostConstant::NSString("r_Attributes"),
    ),
    ("_kSecValueData", HostConstant::NSString("v_Data")),
];
