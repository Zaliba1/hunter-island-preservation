/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Checked one-shot CommonCrypto AES adapter; no guest pointers reach crypto code.
use crate::mem::{ConstVoidPtr, MutPtr, MutVoidPtr};
use crate::Environment;
use aes::cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, KeyInit};
const PARAM: i32 = -4300;
const TOO_SMALL: i32 = -4301;
const MEMORY: i32 = -4302;
const ALIGNMENT: i32 = -4303;
const DECODE: i32 = -4304;
const UNIMPLEMENTED: i32 = -4305;
const LIMIT: usize = 64 * 1024 * 1024;

pub(super) fn transform(
    op: u32,
    options: u32,
    key: &[u8],
    iv: &[u8; 16],
    input: &[u8],
) -> Result<Vec<u8>, i32> {
    if op > 1 || options & !3 != 0 || !matches!(key.len(), 16 | 24 | 32) {
        return Err(PARAM);
    }
    let padding = options & 1 != 0;
    let ecb = options & 2 != 0;
    if (op == 1 || !padding) && input.len() % 16 != 0 {
        return Err(ALIGNMENT);
    }
    if op == 1 && padding && input.is_empty() {
        return Err(DECODE);
    }
    let pad = if op == 0 && padding {
        16 - input.len() % 16
    } else {
        0
    };
    let length = input
        .len()
        .checked_add(pad)
        .filter(|&len| len <= LIMIT)
        .ok_or(MEMORY)?;
    let mut output = Vec::new();
    output.try_reserve_exact(length).map_err(|_| MEMORY)?;
    output.extend_from_slice(input);
    output.resize(length, pad as u8);
    fn blocks<
        C: KeyInit
            + BlockEncrypt
            + BlockDecrypt
            + aes::cipher::BlockSizeUser<BlockSize = aes::cipher::consts::U16>,
    >(
        cipher: C,
        op: u32,
        ecb: bool,
        iv: &[u8; 16],
        data: &mut [u8],
    ) {
        let mut chain = *iv;
        for chunk in data.chunks_exact_mut(16) {
            if op == 0 {
                if !ecb {
                    for (byte, mask) in chunk.iter_mut().zip(chain) {
                        *byte ^= mask;
                    }
                }
                cipher.encrypt_block(GenericArray::from_mut_slice(chunk));
                chain.copy_from_slice(chunk);
            } else {
                let ciphertext: [u8; 16] = chunk.try_into().unwrap();
                cipher.decrypt_block(GenericArray::from_mut_slice(chunk));
                if !ecb {
                    for (byte, mask) in chunk.iter_mut().zip(chain) {
                        *byte ^= mask;
                    }
                }
                chain = ciphertext;
            }
        }
    }
    match key.len() {
        16 => blocks(
            aes::Aes128::new_from_slice(key).unwrap(),
            op,
            ecb,
            iv,
            &mut output,
        ),
        24 => blocks(
            aes::Aes192::new_from_slice(key).unwrap(),
            op,
            ecb,
            iv,
            &mut output,
        ),
        32 => blocks(
            aes::Aes256::new_from_slice(key).unwrap(),
            op,
            ecb,
            iv,
            &mut output,
        ),
        _ => unreachable!(),
    }
    if op == 1 && padding {
        let pad = *output.last().unwrap() as usize;
        if pad == 0
            || pad > 16
            || output[output.len() - pad..]
                .iter()
                .any(|&byte| byte as usize != pad)
        {
            return Err(DECODE);
        }
        output.truncate(output.len() - pad);
    }
    Ok(output)
}

pub(super) fn CCCrypt(
    env: &mut Environment,
    op: u32,
    algorithm: u32,
    options: u32,
    key: ConstVoidPtr,
    key_length: u32,
    iv: ConstVoidPtr,
    input: ConstVoidPtr,
    input_length: u32,
    output: MutVoidPtr,
    available: u32,
    moved: MutPtr<u32>,
) -> i32 {
    // Log scalar metadata only, never keys, IVs, input or output bytes.
    log!("CCCrypt: operation={op} algorithm={algorithm} options={options} key_bytes={key_length} input_bytes={input_length}");
    if !moved.is_null() {
        let Some(bytes) = env.mem.get_bytes_fallible_mut(moved.cast_const().cast(), 4) else {
            return PARAM;
        };
        bytes.copy_from_slice(&0u32.to_le_bytes());
    }
    if algorithm != 0 {
        return UNIMPLEMENTED;
    }
    if op > 1 || options & !3 != 0 || key.is_null() || !matches!(key_length, 16 | 24 | 32) {
        return PARAM;
    }
    if input_length as usize > LIMIT {
        return MEMORY;
    }
    let Some(key) = env.mem.get_bytes_fallible(key, key_length) else {
        return PARAM;
    };
    let mut vector = [0u8; 16];
    if options & 2 == 0 && !iv.is_null() {
        let Some(bytes) = env.mem.get_bytes_fallible(iv, 16) else {
            return PARAM;
        };
        vector.copy_from_slice(bytes);
    }
    if input_length != 0 && input.is_null() {
        return PARAM;
    }
    let input = if input_length == 0 {
        &[][..]
    } else {
        let Some(bytes) = env.mem.get_bytes_fallible(input, input_length) else {
            return PARAM;
        };
        bytes
    };
    let result = match transform(op, options, key, &vector, input) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    let needed = result.len() as u32;
    if available < needed {
        if !moved.is_null() {
            env.mem.write(moved, needed);
        }
        return TOO_SMALL;
    }
    if needed != 0 {
        if output.is_null() {
            return PARAM;
        }
        let Some(destination) = env.mem.get_bytes_fallible_mut(output.cast_const(), needed) else {
            return PARAM;
        };
        destination.copy_from_slice(&result);
    }
    if !moved.is_null() {
        env.mem.write(moved, needed);
    }
    log!("CCCrypt: AES completed; output_bytes={needed}");
    0
}

fn hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn known_vectors() {
    // FIPS-197, appendix C: independent AES-128/192/256 ECB known answers.
    let plain = hex("00112233445566778899aabbccddeeff");
    for (key, expected) in [
        (
            "000102030405060708090a0b0c0d0e0f",
            "69c4e0d86a7b0430d8cdb78070b4c55a",
        ),
        (
            "000102030405060708090a0b0c0d0e0f1011121314151617",
            "dda97ca4864cdfe06eaf70a0ec0d7191",
        ),
        (
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
            "8ea2b7ca516745bfeafc49904b496089",
        ),
    ] {
        let key = hex(key);
        let expected = hex(expected);
        assert_eq!(transform(0, 2, &key, &[0; 16], &plain).unwrap(), expected);
        assert_eq!(transform(1, 2, &key, &[0; 16], &expected).unwrap(), plain);
    }
    // NIST SP800-38A F.2.1: chaining across four different CBC blocks.
    let key = hex("2b7e151628aed2a6abf7158809cf4f3c");
    let iv = hex("000102030405060708090a0b0c0d0e0f").try_into().unwrap();
    let plain = hex(concat!(
        "6bc1bee22e409f96e93d7e117393172a",
        "ae2d8a571e03ac9c9eb76fac45af8e51",
        "30c81c46a35ce411e5fbc1191a0a52ef",
        "f69f2445df4f9b17ad2b417be66c3710"
    ));
    let encrypted = hex(concat!(
        "7649abac8119b246cee98e9b12e9197d",
        "5086cb9b507219ee95db113a917678b2",
        "73bed6b8e3c1743b7116e69e22229516",
        "3ff1caa1681fac09120eca307586e1a7"
    ));
    assert_eq!(transform(0, 0, &key, &iv, &plain).unwrap(), encrypted);
    assert_eq!(transform(1, 0, &key, &iv, &encrypted).unwrap(), plain);
    for options in [1, 3] {
        for length in [0, 1, 15, 16, 17, 32] {
            let plain = vec![0x42; length];
            let encrypted = transform(0, options, &key, &iv, &plain).unwrap();
            assert_eq!(encrypted.len(), (length / 16 + 1) * 16);
            assert_eq!(transform(1, options, &key, &iv, &encrypted).unwrap(), plain);
        }
    }
    assert_eq!(transform(0, 0, &key, &iv, &[0; 1]), Err(ALIGNMENT));
    assert_eq!(transform(1, 1, &key, &iv, &[]), Err(DECODE));
    let bad_padding = transform(0, 2, &key, &iv, &[0; 16]).unwrap();
    assert_eq!(transform(1, 3, &key, &iv, &bad_padding), Err(DECODE));
}
pub(crate) fn guest_probe(env: &mut Environment) {
    known_vectors();
    let key = env.mem.alloc(16);
    env.mem.bytes_at_mut(key.cast(), 16).fill(0);
    let buffer = env.mem.alloc(48);
    env.mem.bytes_at_mut(buffer.cast(), 48).fill(0x51);
    let moved = env.mem.alloc(4).cast::<u32>();
    assert_eq!(
        CCCrypt(
            env,
            0,
            0,
            1,
            key.cast_const(),
            16,
            ConstVoidPtr::null(),
            buffer.cast_const(),
            17,
            buffer,
            48,
            moved
        ),
        0
    );
    assert_eq!(env.mem.read(moved), 32);
    assert_eq!(
        CCCrypt(
            env,
            1,
            0,
            1,
            key.cast_const(),
            16,
            ConstVoidPtr::null(),
            buffer.cast_const(),
            32,
            buffer,
            48,
            moved
        ),
        0
    );
    assert_eq!(env.mem.read(moved), 17);
    assert!(env
        .mem
        .bytes_at(buffer.cast(), 17)
        .iter()
        .all(|&byte| byte == 0x51));
    let before = env.mem.bytes_at(buffer.cast(), 48).to_vec();
    assert_eq!(
        CCCrypt(
            env,
            0,
            0,
            1,
            key.cast_const(),
            16,
            ConstVoidPtr::null(),
            buffer.cast_const(),
            17,
            buffer,
            1,
            moved
        ),
        TOO_SMALL
    );
    assert_eq!(env.mem.read(moved), 32);
    assert_eq!(env.mem.bytes_at(buffer.cast(), 48), before);
    assert_eq!(
        CCCrypt(
            env,
            0,
            0,
            0,
            ConstVoidPtr::null(),
            16,
            ConstVoidPtr::null(),
            buffer.cast_const(),
            16,
            buffer,
            48,
            moved
        ),
        PARAM
    );
    assert_eq!(env.mem.read(moved), 0);
    assert_eq!(
        CCCrypt(
            env,
            0,
            1,
            0,
            key.cast_const(),
            16,
            ConstVoidPtr::null(),
            buffer.cast_const(),
            16,
            buffer,
            48,
            moved
        ),
        UNIMPLEMENTED
    );
    for ptr in [key, buffer, moved.cast()] {
        env.mem.free(ptr);
    }
    echo!("COMMONCRYPTO_AES_VECTORS_PADDING_AND_GUEST_BUFFERS_PASS");
}
#[cfg(test)]
mod tests {
    #[test]
    fn aes_known_answers_modes_padding_and_errors() {
        super::known_vectors();
    }
}
