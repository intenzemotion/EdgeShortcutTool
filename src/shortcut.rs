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
const MANAGED_FEATURE_SWITCH_NAMES: [&str; 2] = ["--enable-features", "--disable-features"];
const PRESERVED_SHORTCUT_SWITCH_NAMES: [&str; 5] = [
    "--profile-directory",
    "--profile-email",
    "--app-id",
    "--app",
    "--user-data-dir",
];

pub struct ComApartment;

impl ComApartment {
    pub fn init() -> WinResult<Self> {
        // ShellLink uses COM, so initialize it once on the UI thread.
        // SAFETY: A successful CoInitializeEx call is balanced by ComApartment::drop
        // on the same UI thread before the app exits.
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }

        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        // SAFETY: ComApartment is only constructed after CoInitializeEx succeeds,
        // so this balances that initialization on the same thread.
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
        let program_data = PathBuf::from(program_data);

        paths.push(program_data.join(r"Microsoft\Windows\Start Menu\Programs\Microsoft Edge.lnk"));
    }

    if let Some(app_data) = env::var_os("AppData") {
        let app_data = PathBuf::from(app_data);

        paths.push(app_data.join(r"Microsoft\Windows\Start Menu\Programs\Microsoft Edge.lnk"));
        paths.push(app_data.join(r"Microsoft\Internet Explorer\Quick Launch\Microsoft Edge.lnk"));
        paths.push(app_data.join(r"Microsoft\Internet Explorer\Quick Launch\User Pinned\StartMenu\Microsoft Edge.lnk"));
        paths.push(app_data.join(r"Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\Microsoft Edge.lnk"));

        add_implicit_edge_shortcuts(&mut paths, &app_data);
    }

    paths.sort();
    paths.dedup();
    paths
}

fn add_implicit_edge_shortcuts(paths: &mut Vec<PathBuf>, app_data: &Path) {
    // Some pinned shortcuts live inside hashed ImplicitAppShortcuts folders.
    let root = app_data.join(r"Microsoft\Internet Explorer\Quick Launch\User Pinned\ImplicitAppShortcuts");

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

    for switch_name in MANAGED_FEATURE_SWITCH_NAMES {
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

pub fn normalize_standalone_options(text: &str) -> String {
    // Standalone switches are entered as normal command-line switches, for example:
    // --force-dark-mode --disable-extensions --mute-audio
    split_argument_tokens(text)
        .into_iter()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn get_custom_options_from_text(standalone_text: &str, enable_text: &str, disable_text: &str) -> String {
    let standalone_options = normalize_standalone_options(standalone_text);
    let enable_features = normalize_feature_list(enable_text);
    let disable_features = normalize_feature_list(disable_text);

    let mut parts = Vec::new();

    if !standalone_options.is_empty() {
        parts.push(standalone_options);
    }

    if !enable_features.is_empty() {
        parts.push(format!("--enable-features=\"{}\"", enable_features));
    }

    if !disable_features.is_empty() {
        parts.push(format!("--disable-features=\"{}\"", disable_features));
    }

    parts.join(" ")
}

pub fn get_standalone_options_from_arguments(arguments: &str) -> String {
    // Prefill Custom Standalone from existing non-feature switches while leaving common
    // shortcut-owned switches, such as profile and app launch arguments, hidden.
    let tokens = split_argument_tokens(arguments);
    let mut items = Vec::new();
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

        if is_preserved_shortcut_switch(token) {
            index = skip_preserved_shortcut_switch(&tokens, index);
            continue;
        }

        if is_switch_token(token) {
            index = push_standalone_switch(&tokens, index, &mut items);
            continue;
        }

        index += 1;
    }

    items.join(" ")
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
        return tokens.get(next_index + 1).map(|value| (value.as_str(), next_index + 2));
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
    // Replace the switches managed by this tool while preserving common shortcut-owned
    // arguments such as profile and app launch arguments.
    let preserved = strip_managed_custom_switches(existing_arguments);
    let managed_options = managed_options.trim();

    match (preserved.is_empty(), managed_options.is_empty()) {
        (true, true) => String::new(),
        (true, false) => managed_options.to_string(),
        (false, true) => preserved,
        (false, false) => format!("{} {}", preserved, managed_options),
    }
}

fn strip_managed_custom_switches(arguments: &str) -> String {
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

        if is_preserved_shortcut_switch(token) {
            index = keep_preserved_shortcut_switch(&tokens, index, &mut kept);
            continue;
        }

        if is_switch_token(token) {
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

fn keep_preserved_shortcut_switch(tokens: &[String], switch_index: usize, kept: &mut Vec<String>) -> usize {
    let token = &tokens[switch_index];
    kept.push(token.clone());

    if !is_exact_preserved_shortcut_switch(token) {
        return switch_index + 1;
    }

    let next_index = switch_index + 1;
    let Some(next) = tokens.get(next_index) else {
        return next_index;
    };

    if next == "=" {
        kept.push(next.clone());

        if let Some(value) = tokens.get(next_index + 1) {
            kept.push(value.clone());
            return next_index + 2;
        }

        return next_index + 1;
    }

    if next.starts_with('=') || !next.starts_with("--") {
        kept.push(next.clone());
        return next_index + 1;
    }

    next_index
}

fn skip_preserved_shortcut_switch(tokens: &[String], switch_index: usize) -> usize {
    let token = &tokens[switch_index];

    if !is_exact_preserved_shortcut_switch(token) {
        return switch_index + 1;
    }

    skip_separated_switch_value(tokens, switch_index)
}

fn push_standalone_switch(tokens: &[String], switch_index: usize, items: &mut Vec<String>) -> usize {
    let token = &tokens[switch_index];
    items.push(token.clone());

    let next_index = switch_index + 1;
    let Some(next) = tokens.get(next_index) else {
        return next_index;
    };

    if next == "=" {
        items.push(next.clone());

        if let Some(value) = tokens.get(next_index + 1) {
            items.push(value.clone());
            return next_index + 2;
        }

        return next_index + 1;
    }

    if next.starts_with('=') || !next.starts_with("--") {
        items.push(next.clone());
        return next_index + 1;
    }

    next_index
}

fn is_switch_token(token: &str) -> bool {
    token.starts_with("--")
}

fn is_managed_feature_switch_with_inline_value(token: &str) -> bool {
    for switch_name in MANAGED_FEATURE_SWITCH_NAMES {
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
    MANAGED_FEATURE_SWITCH_NAMES
        .iter()
        .any(|switch_name| token.eq_ignore_ascii_case(switch_name))
}

fn is_preserved_shortcut_switch(token: &str) -> bool {
    if is_exact_preserved_shortcut_switch(token) {
        return true;
    }

    let lower = token.to_ascii_lowercase();

    for switch_name in PRESERVED_SHORTCUT_SWITCH_NAMES {
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

fn is_exact_preserved_shortcut_switch(token: &str) -> bool {
    PRESERVED_SHORTCUT_SWITCH_NAMES
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

    // SAFETY: COM is initialized before this function is called. The COM wrappers
    // own their interface lifetimes, and the stack buffer lives for GetArguments.
    unsafe {
        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;

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
    // SAFETY: COM is initialized before this function is called. The HSTRING
    // values and stack buffer remain alive for each synchronous COM call.
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

    const STANDALONE_EXAMPLE: &str = "--force-dark-mode --disable-extensions --mute-audio";
    const ENABLE_EXAMPLE: &str = "msForceNoRoundedCornerAndMargin,msDownloadsHub,ParallelDownloading";
    const DISABLE_EXAMPLE: &str = "msShowSignInIndicator,msUndersideButton,MediaRouter";

    #[test]
    fn normalizes_plain_feature_list() {
        let input = " msForceNoRoundedCornerAndMargin, msDownloadsHub ,, ParallelDownloading ";

        assert_eq!(normalize_feature_list(input), ENABLE_EXAMPLE);
    }

    #[test]
    fn normalizes_full_enable_switch() {
        let input = r#"--enable-features="msForceNoRoundedCornerAndMargin, msDownloadsHub""#;

        assert_eq!(normalize_feature_list(input), "msForceNoRoundedCornerAndMargin,msDownloadsHub");
    }

    #[test]
    fn builds_custom_options() {
        let options = get_custom_options_from_text(
            STANDALONE_EXAMPLE,
            "msForceNoRoundedCornerAndMargin, msDownloadsHub, ParallelDownloading",
            r#"--disable-features="msShowSignInIndicator,msUndersideButton,MediaRouter""#,
        );

        let expected = format!(
            r#"{} --enable-features="{}" --disable-features="{}""#,
            STANDALONE_EXAMPLE, ENABLE_EXAMPLE, DISABLE_EXAMPLE
        );

        assert_eq!(options, expected);
    }

    #[test]
    fn extracts_enable_features_from_arguments() {
        let args = format!(
            r#"--profile-directory="Profile 1" --enable-features="{}" --disable-features="msShowSignInIndicator""#,
            "msForceNoRoundedCornerAndMargin,msDownloadsHub"
        );

        assert_eq!(
            get_feature_list_from_arguments(&args, "enable-features"),
            "msForceNoRoundedCornerAndMargin,msDownloadsHub"
        );
    }

    #[test]
    fn extracts_multiple_same_switches() {
        let args = concat!(
            r#"--enable-features=msForceNoRoundedCornerAndMargin "#,
            r#"--enable-features="msDownloadsHub,ParallelDownloading""#
        );

        assert_eq!(get_feature_list_from_arguments(args, "enable-features"), ENABLE_EXAMPLE);
    }

    #[test]
    fn merges_arguments_without_dropping_unrelated_arguments() {
        let existing = concat!(
            r#"--profile-directory="Profile 1" --enable-features="msDownloadsHub" "#,
            r#"--disable-extensions --app-id=abc"#
        );
        let managed = r#"--mute-audio --disable-features="MediaRouter""#;

        assert_eq!(
            merge_shortcut_arguments(existing, managed),
            r#"--profile-directory="Profile 1" --app-id=abc --mute-audio --disable-features="MediaRouter""#
        );
    }

    #[test]
    fn restore_default_removes_custom_switches_but_keeps_shortcut_arguments() {
        let existing = concat!(
            r#"--profile-directory="Default" "#,
            r#"--disable-features="msShowSignInIndicator,MediaRouter" "#,
            r#"--disable-extensions --app-id=abc"#
        );

        assert_eq!(
            merge_shortcut_arguments(existing, ""),
            r#"--profile-directory="Default" --app-id=abc"#
        );
    }

    #[test]
    fn extracts_standalone_options_from_arguments() {
        let args = concat!(
            r#"--profile-directory="Default" --force-dark-mode "#,
            r#"--enable-features="msForceNoRoundedCornerAndMargin" "#,
            r#"--disable-extensions --mute-audio"#
        );

        assert_eq!(get_standalone_options_from_arguments(args), STANDALONE_EXAMPLE);
    }

    #[test]
    fn non_ascii_arguments_do_not_break_tokenization() {
        let existing = r#"--profile-directory="Profilé 1" --enable-features="msDownloadsHub""#;

        assert_eq!(
            merge_shortcut_arguments(existing, r#"--disable-features="MediaRouter""#),
            r#"--profile-directory="Profilé 1" --disable-features="MediaRouter""#
        );
    }
}
