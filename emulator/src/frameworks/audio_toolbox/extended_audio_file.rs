/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `ExtendedAudioFile.h` (Extended Audio File Services)
//!
//! Currently implemented as a dummy wrapper around Audio File Services.

// TODO: Audio format conversion

use super::audio_file::{
    kAudioFileBadPropertySizeError, kAudioFilePropertyDataFormat, kAudioFileReadPermission,
    property_size, AudioFileClose, AudioFileGetProperty, AudioFileID, AudioFileOpenURL,
    AudioFileReadBytes,
};
use super::audio_queue::is_supported_audio_format;
use super::audio_unit::AudioBufferList;
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::carbon_core::{eofErr, paramErr, OSStatus};
use crate::frameworks::core_audio_types::{
    debug_fourcc, fourcc, kAudioFormatLinearPCM, AudioStreamBasicDescription,
};
use crate::frameworks::core_foundation::cf_url::CFURLRef;
use crate::frameworks::foundation::ns_url::to_rust_path;
use crate::mem::{guest_size_of, ConstPtr, ConstVoidPtr, MutPtr, MutVoidPtr, SafeRead};
use crate::Environment;
use std::collections::HashMap;

#[derive(Default)]
pub struct State {
    pub extended_audio_files: HashMap<ExtAudioFileRef, ExtAudioFileHostObject>,
}
impl State {
    pub fn get(framework_state: &mut crate::frameworks::State) -> &mut Self {
        &mut framework_state.audio_toolbox.extended_audio_file
    }
}

pub struct ExtAudioFileHostObject {
    guest_audio_file: AudioFileID,
    client_data_format: Option<AudioStreamBasicDescription>,
    current_bytes_read: i64,
}

#[repr(C, packed)]
pub struct OpaqueExtAudioFile {
    _filler: u8,
}
unsafe impl SafeRead for OpaqueExtAudioFile {}

type ExtAudioFileRef = MutPtr<OpaqueExtAudioFile>;

/// Usually a FourCC.
type ExtAudioFilePropertyID = u32;
const kExtAudioFileProperty_FileDataFormat: ExtAudioFilePropertyID = fourcc(b"ffmt");
const kExtAudioFileProperty_ClientDataFormat: ExtAudioFilePropertyID = fourcc(b"cfmt");
const kExtAudioFileProperty_FileLengthFrames: ExtAudioFilePropertyID = fourcc(b"#frm");

fn ExtAudioFileOpenURL(
    env: &mut Environment,
    in_url: CFURLRef,
    out_ext_audio_file: MutPtr<ExtAudioFileRef>,
) -> OSStatus {
    return_if_null!(in_url);

    let path = to_rust_path(env, in_url);
    log_dbg!(
        "ExtAudioFileOpenURL({:?} '{:?}', {:?})",
        in_url,
        path,
        out_ext_audio_file
    );

    let audio_file_ptr: MutPtr<AudioFileID> = env.mem.alloc(guest_size_of::<AudioFileID>()).cast();
    let res = AudioFileOpenURL(env, in_url, kAudioFileReadPermission, 0, audio_file_ptr);
    let guest_audio_file = env.mem.read(audio_file_ptr);
    env.mem.free(audio_file_ptr.cast());
    if res != 0 {
        log!(
            "ExtAudioFileOpenURL({:?} '{:?}', {:?}) failed, error: {:?}",
            in_url,
            path,
            out_ext_audio_file,
            res
        );
        return res;
    }

    let host_object = ExtAudioFileHostObject {
        guest_audio_file,
        client_data_format: None,
        current_bytes_read: 0,
    };

    let guest_extended_audio_file = env.mem.alloc_and_write(OpaqueExtAudioFile { _filler: 0 });
    State::get(&mut env.framework_state)
        .extended_audio_files
        .insert(guest_extended_audio_file, host_object);

    env.mem.write(out_ext_audio_file, guest_extended_audio_file);

    0 // success
}

fn ExtAudioFileGetProperty(
    env: &mut Environment,
    in_ext_audio_file: ExtAudioFileRef,
    in_property_id: ExtAudioFilePropertyID,
    io_property_data_size: MutPtr<u32>,
    out_property_data: MutVoidPtr,
) -> OSStatus {
    return_if_null!(in_ext_audio_file);
    return_if_null!(io_property_data_size);
    return_if_null!(out_property_data);

    if in_property_id == kExtAudioFileProperty_FileLengthFrames {
        let size = guest_size_of::<i64>();
        if env.mem.read(io_property_data_size) < size {
            return kAudioFileBadPropertySizeError;
        }
        let Some(file) = env
            .framework_state
            .audio_toolbox
            .extended_audio_file
            .extended_audio_files
            .get(&in_ext_audio_file)
        else {
            return paramErr;
        };
        let Some(audio) = env
            .framework_state
            .audio_toolbox
            .audio_file
            .audio_files
            .get(&file.guest_audio_file)
        else {
            return paramErr;
        };
        let audio = &audio.audio_file;
        // AudioFile currently exposes decoded constant-packet PCM for every
        // supported container. Count frames, not channel samples or bytes.
        let frames = audio
            .packet_count()
            .checked_mul(u64::from(audio.audio_description().frames_per_packet))
            .and_then(|n| i64::try_from(n).ok());
        let Some(frames) = frames else {
            return paramErr;
        };
        env.mem.write(out_property_data.cast(), frames);
        env.mem.write(io_property_data_size, size);
        log_dbg!("ExtAudioFile length: {} decoded sample frames", frames);
        return 0;
    }

    let audio_file_property_id = match in_property_id {
        kExtAudioFileProperty_FileDataFormat => kAudioFilePropertyDataFormat,
        _ => unimplemented!(
            "Unimplemented property ID: {}",
            debug_fourcc(in_property_id)
        ),
    };

    let required_size = property_size(audio_file_property_id);
    if env.mem.read(io_property_data_size) != required_size {
        log!("Warning: ExtAudioFileGetProperty() failed");
        return kAudioFileBadPropertySizeError;
    }

    let host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .get(&in_ext_audio_file)
        .unwrap();

    AudioFileGetProperty(
        env,
        host_object.guest_audio_file,
        audio_file_property_id,
        io_property_data_size,
        out_property_data,
    )
}

fn ExtAudioFileSetProperty(
    env: &mut Environment,
    in_ext_audio_file: ExtAudioFileRef,
    in_property_id: ExtAudioFilePropertyID,
    in_property_data_size: u32,
    in_property_data: ConstVoidPtr,
) -> OSStatus {
    return_if_null!(in_ext_audio_file);

    assert_eq!(in_property_id, kExtAudioFileProperty_ClientDataFormat);
    assert_eq!(
        in_property_data_size,
        guest_size_of::<AudioStreamBasicDescription>()
    );

    let audio_desc_ptr: ConstPtr<AudioStreamBasicDescription> = in_property_data.cast();
    let client_audio_desc = env.mem.read(audio_desc_ptr);
    log_dbg!("ExtAudioFileSetProperty {:?}", client_audio_desc);
    let format_id = client_audio_desc.format_id;
    assert_eq!(format_id, kAudioFormatLinearPCM);
    assert!(is_supported_audio_format(&client_audio_desc));

    let host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .get_mut(&in_ext_audio_file)
        .unwrap();
    assert!(host_object.client_data_format.is_none());
    host_object.client_data_format = Some(client_audio_desc);

    let host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .get(&in_ext_audio_file)
        .unwrap();
    let other_host_object = env
        .framework_state
        .audio_toolbox
        .audio_file
        .audio_files
        .get(&host_object.guest_audio_file)
        .unwrap();
    let audio_desc = AudioStreamBasicDescription::from_audio_description(
        other_host_object.audio_file.audio_description(),
    );
    // TODO: support audio format conversions
    assert_eq!(audio_desc, client_audio_desc);

    0 // success
}

fn ExtAudioFileRead(
    env: &mut Environment,
    in_ext_audio_file: ExtAudioFileRef,
    io_number_frames: MutPtr<u32>,
    io_data: MutPtr<AudioBufferList<1>>,
) -> OSStatus {
    return_if_null!(in_ext_audio_file);

    let mut audio_buffer_list = env.mem.read(io_data);
    let num_buffers = audio_buffer_list.number_buffers;
    assert_eq!(num_buffers, 1);

    let host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .get(&in_ext_audio_file)
        .unwrap();

    audio_buffer_list.buffers[0].number_channels =
        host_object.client_data_format.unwrap().channels_per_frame;

    let number_frames = env.mem.read(io_number_frames);
    let bytes_per_frame = host_object.client_data_format.unwrap().bytes_per_frame;
    let number_of_bytes = number_frames.checked_mul(bytes_per_frame).unwrap();
    let number_of_bytes_ptr = env.mem.alloc_and_write(number_of_bytes);
    let res = AudioFileReadBytes(
        env,
        host_object.guest_audio_file,
        false,
        host_object.current_bytes_read,
        number_of_bytes_ptr,
        audio_buffer_list.buffers[0].data,
    );
    let number_of_bytes_read = env.mem.read(number_of_bytes_ptr);
    env.mem.free(number_of_bytes_ptr.cast());
    if res != 0 && res != eofErr {
        log!(
            "ExtAudioFileRead({:?}, {:?}, {:?}) failed, error: {:?}",
            in_ext_audio_file,
            io_number_frames,
            io_data,
            res
        );
        return res;
    }
    env.mem.write(
        io_number_frames,
        number_of_bytes_read.checked_div(bytes_per_frame).unwrap(),
    );
    audio_buffer_list.buffers[0].data_byte_size = number_of_bytes_read;

    let host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .get_mut(&in_ext_audio_file)
        .unwrap();
    host_object.current_bytes_read += number_of_bytes_read as i64;

    env.mem.write(io_data, audio_buffer_list);

    0 // success
}

fn ExtAudioFileDispose(env: &mut Environment, in_ext_audio_file: ExtAudioFileRef) -> OSStatus {
    return_if_null!(in_ext_audio_file);

    let host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .get(&in_ext_audio_file)
        .unwrap();

    let res = AudioFileClose(env, host_object.guest_audio_file);
    assert_eq!(res, 0); // success

    let _host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .remove(&in_ext_audio_file)
        .unwrap();
    env.mem.free(in_ext_audio_file.cast());

    0 // success
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(ExtAudioFileOpenURL(_, _)),
    export_c_func!(ExtAudioFileGetProperty(_, _, _, _)),
    export_c_func!(ExtAudioFileSetProperty(_, _, _, _)),
    export_c_func!(ExtAudioFileRead(_, _, _)),
    export_c_func!(ExtAudioFileDispose(_)),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[test]
    fn decoded_frame_count_and_partial_eof() {
        let mut environment = Environment::new_without_app(
            crate::options::Options {
                headless: true,
                ..Default::default()
            },
            crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1)),
        )
        .unwrap();
        let env = &mut environment;
        for channels in [1, 2] {
            let mut bytes = Cursor::new(Vec::new());
            {
                let mut writer = hound::WavWriter::new(
                    &mut bytes,
                    hound::WavSpec {
                        channels,
                        sample_rate: 22050,
                        bits_per_sample: 16,
                        sample_format: hound::SampleFormat::Int,
                    },
                )
                .unwrap();
                for i in 0..10 * channels {
                    writer.write_sample((i as i16) * 100).unwrap();
                }
                writer.finalize().unwrap();
            }
            let audio_file = crate::audio::AudioFile::read_from_vec(bytes.into_inner()).unwrap();
            let format =
                AudioStreamBasicDescription::from_audio_description(audio_file.audio_description());
            let file_id: AudioFileID = env.mem.alloc(1).cast();
            env.framework_state
                .audio_toolbox
                .audio_file
                .audio_files
                .insert(
                    file_id,
                    super::super::audio_file::AudioFileHostObject { audio_file },
                );
            let extended = env.mem.alloc_and_write(OpaqueExtAudioFile { _filler: 0 });
            State::get(&mut env.framework_state)
                .extended_audio_files
                .insert(
                    extended,
                    ExtAudioFileHostObject {
                        guest_audio_file: file_id,
                        client_data_format: Some(format),
                        current_bytes_read: 0,
                    },
                );
            let size = env.mem.alloc_and_write(16u32);
            let value = env.mem.alloc_and_write(-1i64);
            assert_eq!(
                ExtAudioFileGetProperty(
                    env,
                    extended,
                    kExtAudioFileProperty_FileLengthFrames,
                    size,
                    value.cast()
                ),
                0
            );
            assert_eq!(env.mem.read(size), 8);
            assert_eq!(env.mem.read(value), 10);
            env.mem.write(size, 4u32);
            env.mem.write(value, -1i64);
            assert_eq!(
                ExtAudioFileGetProperty(
                    env,
                    extended,
                    kExtAudioFileProperty_FileLengthFrames,
                    size,
                    value.cast()
                ),
                kAudioFileBadPropertySizeError
            );
            assert_eq!(env.mem.read(value), -1);
            let buffer = env.mem.alloc(20 * format.bytes_per_frame);
            let list = env.mem.alloc_and_write(AudioBufferList {
                number_buffers: 1,
                buffers: [super::super::audio_unit::AudioBuffer {
                    number_channels: channels.into(),
                    data_byte_size: 20 * format.bytes_per_frame,
                    data: buffer,
                }],
            });
            let frames = env.mem.alloc_and_write(20u32);
            assert_eq!(ExtAudioFileRead(env, extended, frames, list), 0);
            assert_eq!(env.mem.read(frames), 10); // final partial read is not dropped
            let returned_bytes = env.mem.read(list).buffers[0].data_byte_size;
            assert_eq!(returned_bytes, 10 * format.bytes_per_frame);
            assert_eq!(&env.mem.bytes_at(buffer.cast(), 4)[..], &[0, 0, 100, 0]);
            env.mem.write(frames, 20u32);
            assert_eq!(ExtAudioFileRead(env, extended, frames, list), 0);
            assert_eq!(env.mem.read(frames), 0);
            env.mem.write(size, 8u32);
            assert_eq!(
                ExtAudioFileGetProperty(
                    env,
                    extended,
                    kExtAudioFileProperty_FileLengthFrames,
                    size,
                    value.cast()
                ),
                0
            );
            assert_eq!(env.mem.read(value), 10); // total length independent of cursor
            assert_eq!(ExtAudioFileDispose(env, extended), 0);
            for allocation in [
                buffer,
                size.cast(),
                value.cast(),
                list.cast(),
                frames.cast(),
            ] {
                env.mem.free(allocation);
            }
        }
    }
}
