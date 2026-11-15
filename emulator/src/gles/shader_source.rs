/* SPDX-License-Identifier: MPL-2.0 */
//! Compatibility for ES compilers requiring extension directives in the preamble.
//! Only unconditional, single-line directives are moved. Conditional branches
//! and continued directives are retained without changing their activation.

fn uncomment(line: &[u8], in_block: &mut bool) -> Vec<u8> {
    let mut out=Vec::new();let mut i=0;
    while i<line.len() {
        if *in_block {
            if line[i..].starts_with(b"*/"){*in_block=false;i+=2;}else{i+=1;}
        }else if line[i..].starts_with(b"//"){break;}
        else if line[i..].starts_with(b"/*"){*in_block=true;i+=2;out.push(b' ');}
        else{out.push(line[i]);i+=1;}
    }
    out
}
fn trim_start(bytes:&[u8])->&[u8]{let n=bytes.iter().take_while(|b|b.is_ascii_whitespace()).count();&bytes[n..]}

/// Apple's desktop GL compiler accepts GLSL 1.20 rather than ESSL 1.00.
/// Preserve GL_ES branches and line locations; only lower the ES precision
/// syntax and the derivative extension already provided by desktop GLSL.
#[cfg(any(target_os = "macos", test))]
pub fn macos_source(source: &[u8]) -> Vec<u8> {
    let mut masked = source.to_vec();
    let mut block = false;
    let mut line = false;
    let mut i = 0;
    while i < masked.len() {
        if block {
            if source[i..].starts_with(b"*/") {
                masked[i..i+2].fill(b' '); block = false; i += 2; continue;
            }
            if masked[i] != b'\n' { masked[i] = b' '; }
        } else if line {
            if masked[i] == b'\n' { line = false; } else { masked[i] = b' '; }
        } else if source[i..].starts_with(b"/*") {
            masked[i..i+2].fill(b' '); block = true; i += 2; continue;
        } else if source[i..].starts_with(b"//") {
            masked[i..i+2].fill(b' '); line = true; i += 2; continue;
        }
        i += 1;
    }
    let mut out = source.to_vec();
    // Blank precision statements while keeping newlines and commented text.
    i = 0;
    while i < masked.len() {
        if (i == 0 || !(masked[i-1].is_ascii_alphanumeric() || masked[i-1] == b'_'))
            && masked[i..].starts_with(b"precision")
            && masked.get(i+9).is_some_and(|b| b.is_ascii_whitespace())
        {
            if let Some(end) = masked[i..].iter().position(|&b| b == b';') {
                let words: Vec<_> = masked[i..i+end].split(|b| b.is_ascii_whitespace()).filter(|s| !s.is_empty()).collect();
                if words.len() == 3 && matches!(words[1], b"lowp" | b"mediump" | b"highp") {
                    for n in i..=i+end { if masked[n] != b'\n' && masked[n] != b'\r' && masked[n] != b' ' { out[n] = b' '; } }
                    i += end + 1; continue;
                }
            }
        }
        i += 1;
    }
    let mut result = Vec::new();
    let mut offset = 0;
    for clean in masked.split_inclusive(|&b| b == b'\n') {
        let original = &out[offset..offset+clean.len()];
        let directive = trim_start(clean);
        if directive.starts_with(b"#version") && directive.split(|b| b.is_ascii_whitespace()).nth(1) == Some(b"100") {
            // Retain a leading closed block comment on the #version line.
            let prefix = clean.len()-directive.len();
            result.extend_from_slice(&original[..prefix]);
            result.extend_from_slice(b"#version 120\n#define THLE_GL_ES 1\n#define THLE_FRAGMENT_PRECISION_HIGH 1\n#define THLE_STANDARD_DERIVATIVES 1\n#define lowp\n#define mediump\n#define highp\n");
            let line = source[..offset].iter().filter(|&&b| b == b'\n').count()+2;
            result.extend_from_slice(format!("#line {line}\n").as_bytes());
        } else if directive.starts_with(b"#extension") && directive.split(|b| b.is_ascii_whitespace()).nth(1) == Some(b"GL_OES_standard_derivatives") {
            for &b in original { result.push(if b == b'\n' || b == b'\r' { b } else { b' ' }); }
        } else {
            // Desktop GLSL reserves macro names beginning with GL_. Rename
            // the ES feature tests without changing commented identifiers.
            let mut j = 0;
            while j < clean.len() {
                if clean[j].is_ascii_alphabetic() || clean[j] == b'_' {
                    let end = j + clean[j..].iter().take_while(|&&b| b.is_ascii_alphanumeric() || b == b'_').count();
                    let replacement: Option<&[u8]> = match &clean[j..end] {
                        b"GL_ES" => Some(b"THLE_GL_ES"),
                        b"GL_FRAGMENT_PRECISION_HIGH" => Some(b"THLE_FRAGMENT_PRECISION_HIGH"),
                        b"GL_OES_standard_derivatives" => Some(b"THLE_STANDARD_DERIVATIVES"),
                        _ => None,
                    };
                    result.extend_from_slice(replacement.unwrap_or(&original[j..end])); j = end;
                } else { result.push(original[j]); j += 1; }
            }
        }
        offset += clean.len();
    }
    result
}

pub fn native_extension_preamble(source:&[u8])->Vec<u8> {
    let lines:Vec<_>=source.split_inclusive(|&b|b==b'\n').collect();
    let mut block=false;let mut depth=0usize;let mut version=None;let mut extensions=Vec::new();
    for (index,line) in lines.iter().enumerate() {
        let began_in_block=block;
        let clean=uncomment(line,&mut block);let clean=trim_start(&clean);
        if !clean.starts_with(b"#"){continue;}
        let directive=trim_start(&clean[1..]);
        let end=directive.iter().take_while(|b|b.is_ascii_alphabetic()).count();
        match &directive[..end] {
            b"version" if depth==0=>version=Some(index),
            b"if"|b"ifdef"|b"ifndef"=>depth+=1,
            b"endif"=>depth=depth.saturating_sub(1),
            b"extension" if depth==0 && version.is_some() && !began_in_block && !block && !line.iter().rev().skip_while(|b|b.is_ascii_whitespace()).next().is_some_and(|b|*b==b'\\')=>extensions.push(index),
            _=>{},
        }
    }
    let Some(version)=version else{return source.to_vec()};
    if extensions.is_empty(){return source.to_vec();}
    let mut out=Vec::with_capacity(source.len()+64);
    for (index,line) in lines.iter().enumerate() {
        if extensions.contains(&index){if line.ends_with(b"\n"){out.push(b'\n');}continue;}
        out.extend_from_slice(line);
        if index==version {
            if !out.ends_with(b"\n"){out.push(b'\n');}
            for &extension in &extensions {
                out.extend_from_slice(lines[extension]);
                if !out.ends_with(b"\n"){out.push(b'\n');}
            }
            out.extend_from_slice(format!("#line {}\n",version+2).as_bytes());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn macos_lowers_es_precision_without_touching_comments_or_identifiers() {
        let input = b"// precision highp float;\n#version 100\n#ifdef GL_ES\nprecision mediump float;\n#endif\nvarying lowp vec4 c; void main(){gl_FragColor=c;}\n";
        let out = macos_source(input);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("#version 120\n#define THLE_GL_ES 1"));
        assert!(text.starts_with("// precision highp float;\n"));
        assert!(!text.contains("precision mediump float;"));
        assert!(text.contains("varying lowp vec4 c;"));
    }
    #[test] fn macos_retains_conditional_structure_and_unknown_extensions() {
        let input = b"#version 100\n#ifdef GL_ES\n#extension GL_OES_standard_derivatives : enable\n#endif\n#extension UNKNOWN : require\n/* precision highp float; */\nvoid main(){float precision_value=1.;}\n";
        let out = String::from_utf8(macos_source(input)).unwrap();
        assert!(out.contains("#ifdef THLE_GL_ES\n"));
        assert!(out.contains("#extension UNKNOWN : require"));
        assert!(out.contains("/* precision highp float; */"));
        assert!(out.contains("precision_value=1."));
        assert!(!out.contains("#extension GL_OES_standard_derivatives"));
    }
    #[test]fn late_derivative_directive_precedes_precision_and_uniforms(){
        let input=b"#version 100\nprecision mediump float;\nuniform float width;\n#extension GL_OES_standard_derivatives : enable\nvoid main(){}\n";
        let result=native_extension_preamble(input);
        assert!(result.starts_with(b"#version 100\n#extension GL_OES_standard_derivatives : enable\n#line 2\nprecision"));
        assert!(result.ends_with(b"\n\nvoid main(){}\n"));
    }
    #[test]fn inactive_directives_are_not_unconditionally_enabled(){
        let input=b"#version 100\nprecision mediump float;\n#if 0\n#extension UNKNOWN : require\n#endif\n";
        assert_eq!(native_extension_preamble(input),input);
    }
    #[test]fn comments_are_not_directives(){
        let input=b"#version 100\n/*\n#extension FAKE : require\n*/\n// #extension FAKE2 : require\nvoid main(){}\n";
        assert_eq!(native_extension_preamble(input),input);
    }
    #[test]fn real_version_after_comments_and_directive_order_are_preserved(){
        let input=b"/* heading */\n#version 100\nprecision mediump float;\n#extension FIRST : enable\n#extension SECOND : enable\n";
        assert!(native_extension_preamble(input).starts_with(b"/* heading */\n#version 100\n#extension FIRST : enable\n#extension SECOND : enable\n#line 3\n"));
    }
    #[test]fn continued_or_versionless_sources_remain_unchanged(){
        let input=b"#version 100\n#extension LONG : \\\nenable\n";
        assert_eq!(native_extension_preamble(input),input);
        assert_eq!(native_extension_preamble(b"void main(){}"),b"void main(){}");
    }
    #[test]fn cross_line_comments_are_not_moved_out_of_context(){
        for source in [b"#version 100\n/* comment\n*/ #extension X : enable\n".as_slice(),b"#version 100\n#extension X : enable /* comment\nend */\n".as_slice()]{
            assert_eq!(native_extension_preamble(source),source);
        }
    }
}
