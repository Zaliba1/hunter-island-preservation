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
