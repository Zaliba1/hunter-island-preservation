/* SPDX-License-Identifier: MPL-2.0 */
//! Read-only, offline save-history inspector. Requires the user's own IPA.
use std::path::PathBuf;
fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut ipa = None;
    let mut save = None;
    let mut output = None;
    while let Some(arg) = args.next() {
        let value = match arg.as_str() {
            "--ipa" | "--save" | "--json" => args.next().ok_or("Missing argument value")?,
            "--help" => {
                println!("hunter-history --ipa YOUR_GAME.ipa --save sv_123 [--json report.json]\nReads only. Local metadata is not authenticated anti-cheat evidence.");
                return Ok(());
            }
            _ => return Err("Use --help for usage".into()),
        };
        match arg.as_str() {
            "--ipa" => ipa = Some(PathBuf::from(value)),
            "--save" => save = Some(PathBuf::from(value)),
            _ => output = Some(PathBuf::from(value)),
        }
    }
    let ipa = ipa.ok_or("Supply your own compatible game IPA with --ipa")?;
    let save = save.ok_or("Choose a game save with --save")?;
    let report = touchHLE::preservation::inspect_file(&ipa, &save)?;
    let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    if let Some(path) = output {
        // Never overwrite the save, IPA, or any existing file.
        use std::io::Write;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?
            .write_all(text.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    println!("{text}");
    Ok(())
}
