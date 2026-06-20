use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use windows::core::{HSTRING, Interface, Result as WinResult};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED, STGM,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

const ARGUMENT_BUFFER_LEN: usize = 32_768;
const EDGE_SHORTCUT_NAME: &str = "Microsoft Edge.lnk";

pub struct ComApartment;

impl ComApartment {
    pub fn init() -> WinResult<Self> {
        // ShellLink uses COM, so initialize it once on the UI thread.
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }

        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

#[derive(Clone)]
pub struct EdgeExecutableCandidate {
    pub source: &'static str,
    pub path: Option<PathBuf>,
    pub exists: bool,
    pub selected: bool,
}

#[derive(Clone)]
pub enum ShortcutApplyState {
    Updated,
    Failed,
    IgnoredMissing,
}

#[derive(Clone)]
pub struct ShortcutApplyDetail {
    pub path: PathBuf,
    pub state: ShortcutApplyState,
}

pub struct ApplyResult {
    pub found_shortcuts: usize,
    pub updated: usize,
    pub failed: usize,
    pub details: Vec<ShortcutApplyDetail>,
}

pub fn get_edge_executable_candidates() -> Vec<EdgeExecutableCandidate> {
    // Used by the Info window only; existing shortcut target paths are preserved.
    let mut candidates = Vec::new();

    for (source, variable_name) in [
        ("ProgramFiles(x86)", "ProgramFiles(x86)"),
        ("ProgramFiles", "ProgramFiles"),
        ("LocalAppData", "LocalAppData"),
    ] {
        let path = env::var_os(variable_name)
            .map(|base| PathBuf::from(base).join(r"Microsoft\Edge\Application\msedge.exe"));

        let exists = path.as_ref().map(|path| path.exists()).unwrap_or(false);

        candidates.push(EdgeExecutableCandidate {
            source,
            path,
            exists,
            selected: false,
        });
    }

    let mut selected = false;

    for candidate in &mut candidates {
        if !selected && candidate.exists {
            candidate.selected = true;
            selected = true;
        }
    }

    candidates
}

pub fn get_shortcut_paths() -> Vec<PathBuf> {
    // Keep missing paths in the list so Info can show what was checked.
    let mut paths = Vec::new();

    if let Some(public) = env::var_os("Public") {
        paths.push(PathBuf::from(public).join(r"Desktop\Microsoft Edge.lnk"));
    }

    if let Some(user_profile) = env::var_os("UserProfile") {
        paths.push(PathBuf::from(user_profile).join(r"Desktop\Microsoft Edge.lnk"));
    }

    if let Some(program_data) = env::var_os("ProgramData") {
        paths.push(
            PathBuf::from(program_data)
                .join(r"Microsoft\Windows\Start Menu\Programs\Microsoft Edge.lnk"),
        );
    }

    if let Some(app_data) = env::var_os("AppData") {
        let app_data = PathBuf::from(app_data);

        paths.push(app_data.join(r"Microsoft\Windows\Start Menu\Programs\Microsoft Edge.lnk"));

        paths.push(app_data.join(r"Microsoft\Internet Explorer\Quick Launch\Microsoft Edge.lnk"));

        paths.push(app_data.join(
            r"Microsoft\Internet Explorer\Quick Launch\User Pinned\StartMenu\Microsoft Edge.lnk",
        ));

        paths.push(app_data.join(
            r"Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\Microsoft Edge.lnk",
        ));

        add_implicit_edge_shortcuts(&mut paths, &app_data);
    }

    paths.sort();
    paths.dedup();
    paths
}

fn add_implicit_edge_shortcuts(paths: &mut Vec<PathBuf>, app_data: &Path) {
    // Some pinned shortcuts live inside hashed ImplicitAppShortcuts folders.
    let root = app_data.join(
        r"Microsoft\Internet Explorer\Quick Launch\User Pinned\ImplicitAppShortcuts",
    );

    let Ok(hash_dirs) = fs::read_dir(root) else {
        return;
    };

    for hash_dir in hash_dirs.flatten() {
        let hash_dir_path = hash_dir.path();

        if !hash_dir_path.is_dir() {
            continue;
        }

        let Ok(shortcuts) = fs::read_dir(hash_dir_path) else {
            continue;
        };

        for shortcut in shortcuts.flatten() {
            let shortcut_path = shortcut.path();

            if is_edge_shortcut_file(&shortcut_path) {
                paths.push(shortcut_path);
            }
        }
    }
}

fn is_edge_shortcut_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };

    file_name.eq_ignore_ascii_case(EDGE_SHORTCUT_NAME)
}

fn strip_optional_feature_switch(text: &str) -> &str {
    let trimmed = text.trim();

    for switch_name in ["--enable-features", "--disable-features"] {
        let Some(head) = trimmed.get(..switch_name.len()) else {
            continue;
        };

        if !head.eq_ignore_ascii_case(switch_name) {
            continue;
        }

        let rest = trimmed.get(switch_name.len()..).unwrap_or("").trim_start();

        if let Some(after_equals) = rest.strip_prefix('=') {
            return after_equals.trim_start();
        }
    }

    trimmed
}

fn strip_matching_quotes(text: &str) -> &str {
    let bytes = text.as_bytes();

    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];

        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &text[1..text.len() - 1];
        }
    }

    text
}

pub fn normalize_feature_list(text: &str) -> String {
    // Accept either a plain comma list or a full Edge feature switch.
    let value = strip_optional_feature_switch(text).trim();
    let value = strip_matching_quotes(value).trim();

    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

pub fn get_custom_options_from_text(enable_text: &str, disable_text: &str) -> String {
    let enable_features = normalize_feature_list(enable_text);
    let disable_features = normalize_feature_list(disable_text);

    let mut parts = Vec::new();

    if !enable_features.is_empty() {
        parts.push(format!("--enable-features=\"{}\"", enable_features));
    }

    if !disable_features.is_empty() {
        parts.push(format!("--disable-features=\"{}\"", disable_features));
    }

    parts.join(" ")
}

pub fn get_feature_list_from_arguments(arguments: &str, switch_name: &str) -> String {
    // Read existing shortcut flags back into the Custom fields.
    let needle = format!("--{}", switch_name.to_ascii_lowercase());
    let tokens = split_argument_tokens(arguments);
    let mut items = Vec::new();
    let mut index = 0usize;

    while index < tokens.len() {
        let token = &tokens[index];
        let lower = token.to_ascii_lowercase();

        if lower == needle {
            let Some((value, next_index)) = get_separated_switch_value(&tokens, index) else {
                index += 1;
                continue;
            };

            let clean = normalize_feature_list(value);

            if !clean.is_empty() {
                items.push(clean);
            }

            index = next_index;
            continue;
        }

        if lower.starts_with(&needle) {
            let Some(rest) = token.get(needle.len()..) else {
                index += 1;
                continue;
            };

            if let Some(value) = rest.strip_prefix('=') {
                let clean = normalize_feature_list(value);

                if !clean.is_empty() {
                    items.push(clean);
                }
            }
        }

        index += 1;
    }

    items.join(",")
}

fn get_separated_switch_value(tokens: &[String], switch_index: usize) -> Option<(&str, usize)> {
    let next_index = switch_index + 1;
    let next = tokens.get(next_index)?;

    if next == "=" {
        return tokens
            .get(next_index + 1)
            .map(|value| (value.as_str(), next_index + 2));
    }

    if let Some(value) = next.strip_prefix('=') {
        return Some((value, next_index + 1));
    }

    if next.starts_with("--") {
        return None;
    }

    Some((next.as_str(), next_index + 1))
}

pub fn merge_shortcut_arguments(existing_arguments: &str, managed_options: &str) -> String {
    // Replace only the Edge feature switches managed by this tool.
    let preserved = strip_managed_feature_switches(existing_arguments);
    let managed_options = managed_options.trim();

    match (preserved.is_empty(), managed_options.is_empty()) {
        (true, true) => String::new(),
        (true, false) => managed_options.to_string(),
        (false, true) => preserved,
        (false, false) => format!("{} {}", preserved, managed_options),
    }
}

fn strip_managed_feature_switches(arguments: &str) -> String {
    let tokens = split_argument_tokens(arguments);
    let mut kept = Vec::new();
    let mut index = 0usize;

    while index < tokens.len() {
        let token = &tokens[index];

        if is_managed_feature_switch_with_inline_value(token) {
            index += 1;
            continue;
        }

        if is_exact_managed_feature_switch(token) {
            index = skip_separated_switch_value(&tokens, index);
            continue;
        }

        kept.push(token.clone());
        index += 1;
    }

    kept.join(" ")
}

fn skip_separated_switch_value(tokens: &[String], switch_index: usize) -> usize {
    let next_index = switch_index + 1;
    let Some(next) = tokens.get(next_index) else {
        return next_index;
    };

    if next == "=" {
        if tokens.get(next_index + 1).is_some() {
            return next_index + 2;
        }

        return next_index + 1;
    }

    if next.starts_with('=') || !next.starts_with("--") {
        return next_index + 1;
    }

    next_index
}

fn is_managed_feature_switch_with_inline_value(token: &str) -> bool {
    for switch_name in ["--enable-features", "--disable-features"] {
        let lower = token.to_ascii_lowercase();

        if !lower.starts_with(switch_name) {
            continue;
        }

        let Some(rest) = lower.get(switch_name.len()..) else {
            continue;
        };

        if rest.starts_with('=') {
            return true;
        }
    }

    false
}

fn is_exact_managed_feature_switch(token: &str) -> bool {
    ["--enable-features", "--disable-features"]
        .iter()
        .any(|switch_name| token.eq_ignore_ascii_case(switch_name))
}

fn split_argument_tokens(arguments: &str) -> Vec<String> {
    // Small tokenizer that keeps quoted shortcut arguments together.
    let mut tokens = Vec::new();
    let mut start = None;
    let mut quote = None;

    for (index, ch) in arguments.char_indices() {
        if start.is_none() {
            if ch.is_whitespace() {
                continue;
            }

            start = Some(index);
        }

        match quote {
            Some(current_quote) => {
                if ch == current_quote {
                    quote = None;
                }
            }
            None => {
                if ch == '"' || ch == '\'' {
                    quote = Some(ch);
                } else if ch.is_whitespace() {
                    if let Some(token_start) = start.take() {
                        tokens.push(arguments[token_start..index].to_string());
                    }
                }
            }
        }
    }

    if let Some(token_start) = start {
        tokens.push(arguments[token_start..].to_string());
    }

    tokens
}

fn read_shortcut_arguments(path: &Path) -> Option<String> {
    // Used to prefill the Custom window from the first shortcut with arguments.
    if !path.exists() {
        return None;
    }

    unsafe {
        let shell_link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;

        let persist_file: IPersistFile = shell_link.cast().ok()?;
        let path_string = path.to_string_lossy().to_string();
        let path_hstring = HSTRING::from(path_string);

        persist_file.Load(&path_hstring, STGM(0)).ok()?;

        let mut buffer = [0u16; ARGUMENT_BUFFER_LEN];

        shell_link.GetArguments(&mut buffer).ok()?;

        let len = buffer
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(buffer.len());

        let arguments = String::from_utf16_lossy(&buffer[..len]);

        if arguments.trim().is_empty() {
            None
        } else {
            Some(arguments)
        }
    }
}

pub fn get_current_shortcut_arguments() -> String {
    for path in get_shortcut_paths() {
        if let Some(arguments) = read_shortcut_arguments(&path) {
            return arguments;
        }
    }

    String::new()
}

fn update_shortcut_arguments(path: &Path, managed_options: &str) -> WinResult<()> {
    // Load the existing shortcut and change only its argument string.
    unsafe {
        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        let persist_file: IPersistFile = shell_link.cast()?;
        let path_string = path.to_string_lossy().to_string();
        let path_hstring = HSTRING::from(path_string);

        persist_file.Load(&path_hstring, STGM(0))?;

        let mut buffer = [0u16; ARGUMENT_BUFFER_LEN];
        shell_link.GetArguments(&mut buffer)?;

        let len = buffer
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(buffer.len());

        let existing_arguments = String::from_utf16_lossy(&buffer[..len]);
        let merged_arguments = merge_shortcut_arguments(&existing_arguments, managed_options);

        shell_link.SetArguments(&HSTRING::from(merged_arguments))?;
        persist_file.Save(&path_hstring, true)?;
    }

    Ok(())
}

pub fn apply_options(options: &str) -> ApplyResult {
    // Store per-shortcut results so the Info window can explain what happened.
    let mut result = ApplyResult {
        found_shortcuts: 0,
        updated: 0,
        failed: 0,
        details: Vec::new(),
    };

    for shortcut_path in get_shortcut_paths() {
        if !shortcut_path.exists() {
            result.details.push(ShortcutApplyDetail {
                path: shortcut_path,
                state: ShortcutApplyState::IgnoredMissing,
            });
            continue;
        }

        result.found_shortcuts += 1;

        if update_shortcut_arguments(&shortcut_path, options).is_ok() {
            result.updated += 1;
            result.details.push(ShortcutApplyDetail {
                path: shortcut_path,
                state: ShortcutApplyState::Updated,
            });
        } else {
            result.failed += 1;
            result.details.push(ShortcutApplyDetail {
                path: shortcut_path,
                state: ShortcutApplyState::Failed,
            });
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_plain_feature_list() {
        assert_eq!(
            normalize_feature_list(" FeatureA, FeatureB ,, FeatureC "),
            "FeatureA,FeatureB,FeatureC"
        );
    }

    #[test]
    fn normalizes_full_enable_switch() {
        assert_eq!(
            normalize_feature_list(r#"--enable-features="FeatureA, FeatureB""#),
            "FeatureA,FeatureB"
        );
    }

    #[test]
    fn builds_custom_options() {
        assert_eq!(
            get_custom_options_from_text("FeatureA, FeatureB", r#"--disable-features="FeatureC""#),
            r#"--enable-features="FeatureA,FeatureB" --disable-features="FeatureC""#
        );
    }

    #[test]
    fn extracts_enable_features_from_arguments() {
        let args = r#"--profile-directory="Profile 1" --enable-features="A,B" --disable-features="C""#;

        assert_eq!(get_feature_list_from_arguments(args, "enable-features"), "A,B");
    }

    #[test]
    fn extracts_multiple_same_switches() {
        let args = r#"--enable-features=A --enable-features="B,C""#;

        assert_eq!(get_feature_list_from_arguments(args, "enable-features"), "A,B,C");
    }

    #[test]
    fn merges_arguments_without_dropping_unrelated_arguments() {
        let existing = r#"--profile-directory="Profile 1" --enable-features="Old" --app-id=abc"#;
        let managed = r#"--disable-features="New""#;

        assert_eq!(
            merge_shortcut_arguments(existing, managed),
            r#"--profile-directory="Profile 1" --app-id=abc --disable-features="New""#
        );
    }

    #[test]
    fn restore_default_removes_feature_switches_but_keeps_other_arguments() {
        let existing = r#"--profile-directory="Default" --disable-features="A,B" --flag"#;

        assert_eq!(
            merge_shortcut_arguments(existing, ""),
            r#"--profile-directory="Default" --flag"#
        );
    }

    #[test]
    fn non_ascii_arguments_do_not_break_tokenization() {
        let existing = r#"--profile-directory="Profilé 1" --enable-features="A""#;

        assert_eq!(
            merge_shortcut_arguments(existing, r#"--disable-features="B""#),
            r#"--profile-directory="Profilé 1" --disable-features="B""#
        );
    }
}
