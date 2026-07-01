use std::env;
use std::fs;
use std::path::{
    Path, PathBuf
};
use std::process::{
    Command, Output
};
use std::os::windows::process::CommandExt;

use windows::core::{
    HSTRING, Interface, Result as WinResult
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM
};
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, IShellLinkW, KF_FLAG_DEFAULT, SHGetKnownFolderPath, ShellLink
};

const ARGUMENT_BUFFER_LEN: usize = 32_768;
const CREATE_NO_WINDOW: u32 = 0x08000000;
const MANAGED_FEATURE_SWITCH_NAMES: [&str; 2] = ["--enable-features", "--disable-features"];
const APP_REGISTRY_KEY: &str = r"HKCU\Software\EdgeShortcutTool";
const EXTERNAL_LINK_COMMAND_KEY_HKCU: &str = r"HKCU\Software\Classes\MSEdgeHTM\shell\open\command";
const EXTERNAL_LINK_COMMAND_KEY_HKLM: &str = r"HKLM\Software\Classes\MSEdgeHTM\shell\open\command";
const EXTERNAL_LINK_ARGUMENT_SUFFIX: &str = "--single-argument %1";
const EXTERNAL_ORIGINAL_COMMAND_VALUE: &str = "ExternalOriginalCommand";
const EXTERNAL_ORIGINAL_COMMAND_SAVED_VALUE: &str = "ExternalOriginalCommandSaved";
const EXTERNAL_LAST_COMMAND_VALUE: &str = "ExternalLastCommand";
const SETTING_HIDE_SIGN_IN_INDICATOR: &str = "HideSignInIndicator";
const SETTING_RESTORE_SIDEBAR: &str = "RestoreSidebar";
const SETTING_DISABLE_EXTENSIONS: &str = "DisableExtensions";
const SETTING_APPLY_EXTERNAL_LINKS: &str = "ApplyExternalLinks";
const SETTING_SHORTCUT_STABLE: &str = "ShortcutStable";
const SETTING_SHORTCUT_BETA: &str = "ShortcutBeta";
const SETTING_SHORTCUT_DEV: &str = "ShortcutDev";
const SETTING_SHORTCUT_CANARY: &str = "ShortcutCanary";

#[derive(Clone, Copy)]
pub struct ShortcutTarget {
    pub display_name: &'static str,
    pub shortcut_name: &'static str
}

pub const SHORTCUT_TARGETS: [ShortcutTarget; 4] = [
    ShortcutTarget {
        display_name: "Microsoft Edge",
        shortcut_name: "Microsoft Edge.lnk"
    },
    ShortcutTarget {
        display_name: "Microsoft Edge Beta",
        shortcut_name: "Microsoft Edge Beta.lnk"
    },
    ShortcutTarget {
        display_name: "Microsoft Edge Dev",
        shortcut_name: "Microsoft Edge Dev.lnk"
    },
    ShortcutTarget {
        display_name: "Microsoft Edge Canary",
        shortcut_name: "Microsoft Edge Canary.lnk"
    }
];

#[derive(Clone)]
pub struct ShortcutTargetSelection {
    pub stable: bool,
    pub beta: bool,
    pub dev: bool,
    pub canary: bool
}

impl Default for ShortcutTargetSelection {
    fn default() -> Self {
        Self {
            stable: true,
            beta: false,
            dev: false,
            canary: false
        }
    }
}

impl ShortcutTargetSelection {
    pub fn selected_targets(&self) -> Vec<ShortcutTarget> {
        let mut targets = Vec::new();

        if self.stable {
            targets.push(SHORTCUT_TARGETS[0]);
        }

        if self.beta {
            targets.push(SHORTCUT_TARGETS[1]);
        }

        if self.dev {
            targets.push(SHORTCUT_TARGETS[2]);
        }

        if self.canary {
            targets.push(SHORTCUT_TARGETS[3]);
        }

        targets
    }

    pub fn unselected_targets(&self) -> Vec<ShortcutTarget> {
        let mut targets = Vec::new();

        if !self.stable {
            targets.push(SHORTCUT_TARGETS[0]);
        }

        if !self.beta {
            targets.push(SHORTCUT_TARGETS[1]);
        }

        if !self.dev {
            targets.push(SHORTCUT_TARGETS[2]);
        }

        if !self.canary {
            targets.push(SHORTCUT_TARGETS[3]);
        }

        targets
    }
}

#[derive(Clone)]
pub struct AppSettings {
    pub hide_sign_in_indicator: bool,
    pub restore_sidebar: bool,
    pub disable_extensions: bool,
    pub apply_external_links: bool,
    pub shortcut_selection: ShortcutTargetSelection
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            hide_sign_in_indicator: true,
            restore_sidebar: false,
            disable_extensions: false,
            apply_external_links: false,
            shortcut_selection: ShortcutTargetSelection::default()
        }
    }
}

pub fn selected_shortcut_display_names(selection: &ShortcutTargetSelection) -> Vec<&'static str> {
    selection
        .selected_targets()
        .into_iter()
        .map(|target| target.display_name)
        .collect()
}

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
    pub selected: bool
}

#[derive(Clone)]
pub enum ShortcutApplyState {
    Updated,
    Failed,
    IgnoredMissing
}

#[derive(Clone)]
pub struct ShortcutApplyDetail {
    pub path: PathBuf,
    pub state: ShortcutApplyState
}

#[derive(Clone)]
pub enum ExternalLinkApplyState {
    Updated,
    Restored,
    Failed
}

#[derive(Clone)]
pub struct ExternalLinkApplyResult {
    pub state: ExternalLinkApplyState
}

pub struct ApplyResult {
    pub selected_shortcut_names: Vec<&'static str>,
    pub found_shortcuts: usize,
    pub failed: usize,
    pub details: Vec<ShortcutApplyDetail>,
    pub external_links: Option<ExternalLinkApplyResult>
}

pub fn get_edge_executable_candidates() -> Vec<EdgeExecutableCandidate> {
    // Used by the Info window only; shortcut target paths are not changed.
    let mut candidates = Vec::new();

    for (source, variable_name, relative_path) in [
        ("Stable - ProgramFiles(x86)", "ProgramFiles(x86)", r"Microsoft\Edge\Application\msedge.exe"),
        ("Stable - ProgramFiles", "ProgramFiles", r"Microsoft\Edge\Application\msedge.exe"),
        ("Stable - LocalAppData", "LocalAppData", r"Microsoft\Edge\Application\msedge.exe"),
        ("Beta - ProgramFiles(x86)", "ProgramFiles(x86)", r"Microsoft\Edge Beta\Application\msedge.exe"),
        ("Beta - LocalAppData", "LocalAppData", r"Microsoft\Edge Beta\Application\msedge.exe"),
        ("Dev - ProgramFiles(x86)", "ProgramFiles(x86)", r"Microsoft\Edge Dev\Application\msedge.exe"),
        ("Dev - LocalAppData", "LocalAppData", r"Microsoft\Edge Dev\Application\msedge.exe"),
        ("Canary - ProgramFiles(x86)", "ProgramFiles(x86)", r"Microsoft\Edge SxS\Application\msedge.exe"),
        ("Canary - LocalAppData", "LocalAppData", r"Microsoft\Edge SxS\Application\msedge.exe")
    ] {
        let path = env::var_os(variable_name).map(|base| PathBuf::from(base).join(relative_path));

        let exists = path.as_ref().map(|path| path.exists()).unwrap_or(false);

        candidates.push(EdgeExecutableCandidate {
            source,
            path,
            exists,
            selected: false
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

pub fn get_shortcut_paths(selection: &ShortcutTargetSelection) -> Vec<PathBuf> {
    // Keep missing selected paths in the list so Info can show what was checked.
    get_shortcut_paths_for_targets(&selection.selected_targets())
}

fn get_unselected_shortcut_paths(selection: &ShortcutTargetSelection) -> Vec<PathBuf> {
    get_shortcut_paths_for_targets(&selection.unselected_targets())
}

fn get_shortcut_paths_for_targets(targets: &[ShortcutTarget]) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(public) = env::var_os("Public") {
        let public = PathBuf::from(public);

        for target in targets {
            paths.push(public.join("Desktop").join(target.shortcut_name));
        }
    }

    if let Some(user_profile) = env::var_os("UserProfile") {
        let user_profile = PathBuf::from(user_profile);

        for target in targets {
            paths.push(user_profile.join("Desktop").join(target.shortcut_name));
        }
    }

    add_current_user_desktop_shortcuts(&mut paths, targets);

    if let Some(program_data) = env::var_os("ProgramData") {
        let program_data = PathBuf::from(program_data);

        for target in targets {
            paths.push(program_data.join(r"Microsoft\Windows\Start Menu\Programs").join(target.shortcut_name));
        }
    }

    if let Some(app_data) = env::var_os("AppData") {
        let app_data = PathBuf::from(app_data);

        add_roaming_shortcuts(&mut paths, &app_data, targets);
    }

    if let Some(system_profile_app_data) = get_system_profile_app_data() {
        add_roaming_shortcuts(&mut paths, &system_profile_app_data, targets);
    }

    paths.sort();
    paths.dedup();
    paths
}

fn add_current_user_desktop_shortcuts(paths: &mut Vec<PathBuf>, targets: &[ShortcutTarget]) {
    if let Some(desktop) = get_current_user_desktop_path() {
        for target in targets {
            paths.push(desktop.join(target.shortcut_name));
        }
    }

    for variable_name in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
        let Some(one_drive) = env::var_os(variable_name) else {
            continue;
        };

        let desktop = PathBuf::from(one_drive).join("Desktop");

        for target in targets {
            paths.push(desktop.join(target.shortcut_name));
        }
    }
}

fn get_current_user_desktop_path() -> Option<PathBuf> {
    // Handles redirected desktops, including OneDrive Known Folder Move.
    // SAFETY: SHGetKnownFolderPath returns a CoTaskMem-allocated PWSTR. The string
    // is copied into a Rust String before the original pointer is freed.
    unsafe {
        let path = SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None).ok()?;
        let value = path.to_string().ok();

        CoTaskMemFree(Some(path.as_ptr() as *const core::ffi::c_void));

        value.filter(|value| !value.trim().is_empty()).map(PathBuf::from)
    }
}

fn get_system_profile_app_data() -> Option<PathBuf> {
    let windows_root = env::var_os("SystemRoot").or_else(|| env::var_os("WinDir"))?;

    Some(PathBuf::from(windows_root).join(r"System32\config\systemprofile\AppData\Roaming"))
}

fn add_roaming_shortcuts(paths: &mut Vec<PathBuf>, app_data: &Path, targets: &[ShortcutTarget]) {
    for target in targets {
        paths.push(app_data.join(r"Microsoft\Windows\Start Menu\Programs").join(target.shortcut_name));
        paths.push(app_data.join(r"Microsoft\Internet Explorer\Quick Launch").join(target.shortcut_name));
        paths.push(app_data.join(r"Microsoft\Internet Explorer\Quick Launch\User Pinned\StartMenu").join(target.shortcut_name));
        paths.push(app_data.join(r"Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar").join(target.shortcut_name));
    }

    add_implicit_edge_shortcuts(paths, app_data, targets);
}

fn add_implicit_edge_shortcuts(paths: &mut Vec<PathBuf>, app_data: &Path, targets: &[ShortcutTarget]) {
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

            if is_selected_edge_shortcut_file(&shortcut_path, targets) {
                paths.push(shortcut_path);
            }
        }
    }
}

fn is_selected_edge_shortcut_file(path: &Path, targets: &[ShortcutTarget]) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };

    targets.iter().any(|target| file_name.eq_ignore_ascii_case(target.shortcut_name))
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
    // --disable-extensions --force-dark-mode --mute-audio
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
    // Prefill Custom Standalone from every existing non-feature switch.
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

pub fn merge_shortcut_arguments(_existing_arguments: &str, managed_options: &str) -> String {
    managed_options.trim().to_string()
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
    let lower = token.to_ascii_lowercase();

    for switch_name in MANAGED_FEATURE_SWITCH_NAMES {
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
    MANAGED_FEATURE_SWITCH_NAMES.iter().any(|switch_name| token.eq_ignore_ascii_case(switch_name))
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

fn run_reg(args: &[&str]) -> std::io::Result<Output> {
    Command::new("reg")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
}

fn query_registry_default_value(key: &str) -> Option<String> {
    query_registry_value(key, None)
}

fn query_registry_named_value(key: &str, value_name: &str) -> Option<String> {
    query_registry_value(key, Some(value_name))
}

fn query_registry_value(key: &str, value_name: Option<&str>) -> Option<String> {
    let output = if let Some(value_name) = value_name {
        run_reg(&["query", key, "/v", value_name]).ok()?
    } else {
        run_reg(&["query", key, "/ve"]).ok()?
    };

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    parse_registry_string_value(&text)
}

fn query_registry_bool(key: &str, value_name: &str, default_value: bool) -> bool {
    query_registry_named_value(key, value_name)
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(default_value)
}

fn set_registry_bool(key: &str, value_name: &str, value: bool) -> bool {
    set_registry_named_value(key, value_name, if value { "1" } else { "0" })
}

pub fn load_app_settings() -> AppSettings {
    AppSettings {
        hide_sign_in_indicator: query_registry_bool(APP_REGISTRY_KEY, SETTING_HIDE_SIGN_IN_INDICATOR, true),
        restore_sidebar: query_registry_bool(APP_REGISTRY_KEY, SETTING_RESTORE_SIDEBAR, false),
        disable_extensions: query_registry_bool(APP_REGISTRY_KEY, SETTING_DISABLE_EXTENSIONS, false),
        apply_external_links: query_registry_bool(APP_REGISTRY_KEY, SETTING_APPLY_EXTERNAL_LINKS, false),
        shortcut_selection: ShortcutTargetSelection {
            stable: query_registry_bool(APP_REGISTRY_KEY, SETTING_SHORTCUT_STABLE, true),
            beta: query_registry_bool(APP_REGISTRY_KEY, SETTING_SHORTCUT_BETA, false),
            dev: query_registry_bool(APP_REGISTRY_KEY, SETTING_SHORTCUT_DEV, false),
            canary: query_registry_bool(APP_REGISTRY_KEY, SETTING_SHORTCUT_CANARY, false)
        }
    }
}

pub fn save_app_settings(settings: &AppSettings) {
    let _ = set_registry_bool(APP_REGISTRY_KEY, SETTING_HIDE_SIGN_IN_INDICATOR, settings.hide_sign_in_indicator);
    let _ = set_registry_bool(APP_REGISTRY_KEY, SETTING_RESTORE_SIDEBAR, settings.restore_sidebar);
    let _ = set_registry_bool(APP_REGISTRY_KEY, SETTING_DISABLE_EXTENSIONS, settings.disable_extensions);
    let _ = set_registry_bool(APP_REGISTRY_KEY, SETTING_APPLY_EXTERNAL_LINKS, settings.apply_external_links);
    let _ = set_registry_bool(APP_REGISTRY_KEY, SETTING_SHORTCUT_STABLE, settings.shortcut_selection.stable);
    let _ = set_registry_bool(APP_REGISTRY_KEY, SETTING_SHORTCUT_BETA, settings.shortcut_selection.beta);
    let _ = set_registry_bool(APP_REGISTRY_KEY, SETTING_SHORTCUT_DEV, settings.shortcut_selection.dev);
    let _ = set_registry_bool(APP_REGISTRY_KEY, SETTING_SHORTCUT_CANARY, settings.shortcut_selection.canary);
}

fn parse_registry_string_value(text: &str) -> Option<String> {
    for line in text.lines() {
        let Some(index) = line.find("REG_SZ") else {
            continue;
        };

        let value = line[index + "REG_SZ".len()..].trim();
        return Some(value.to_string());
    }

    None
}

fn set_registry_default_value(key: &str, value: &str) -> bool {
    run_reg(&["add", key, "/ve", "/t", "REG_SZ", "/d", value, "/f"])
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn set_registry_named_value(key: &str, value_name: &str, value: &str) -> bool {
    run_reg(&["add", key, "/v", value_name, "/t", "REG_SZ", "/d", value, "/f"])
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn delete_registry_key(key: &str) -> bool {
    run_reg(&["delete", key, "/f"])
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn delete_registry_named_value(key: &str, value_name: &str) {
    let _ = run_reg(&["delete", key, "/v", value_name, "/f"]);
}

pub fn external_link_command_registry_key() -> &'static str {
    EXTERNAL_LINK_COMMAND_KEY_HKCU
}

fn get_external_link_edge_executable() -> Option<String> {
    query_registry_default_value(EXTERNAL_LINK_COMMAND_KEY_HKLM)
        .and_then(|command| extract_executable_from_command(&command))
        .or_else(|| {
            get_edge_executable_candidates()
                .into_iter()
                .find(|candidate| candidate.selected)
                .and_then(|candidate| candidate.path)
                .map(|path| path.display().to_string())
        })
}

fn extract_executable_from_command(command: &str) -> Option<String> {
    let command = command.trim();

    if let Some(rest) = command.strip_prefix('"') {
        let end = rest.find('"')?;
        return Some(rest[..end].to_string());
    }

    let lower = command.to_ascii_lowercase();
    let end = lower.find(".exe")? + 4;
    Some(command[..end].to_string())
}

fn build_external_link_command(options: &str) -> Option<String> {
    let options = options.trim();

    if options.is_empty() {
        return None;
    }

    let executable = get_external_link_edge_executable()?;

    Some(format!("\"{}\" {} {}", executable, options, EXTERNAL_LINK_ARGUMENT_SUFFIX))
}

fn is_tool_managed_external_link_command(command: &str) -> bool {
    query_registry_named_value(APP_REGISTRY_KEY, EXTERNAL_LAST_COMMAND_VALUE)
        .map(|last_command| last_command == command)
        .unwrap_or(false)
        || command.contains("msForceNoRoundedCornerAndMargin")
        || command.contains("msFeatureGroupNewLookAndFeelHoldout")
        || command.contains("msShowSignInIndicator")
        || command.contains("msHubAppsSidebarRetirement")
}

fn save_external_link_original_command() -> bool {
    if query_registry_named_value(APP_REGISTRY_KEY, EXTERNAL_ORIGINAL_COMMAND_SAVED_VALUE).is_some() {
        return true;
    }

    let saved_original = if let Some(original_command) = query_registry_default_value(EXTERNAL_LINK_COMMAND_KEY_HKCU) {
        if is_tool_managed_external_link_command(&original_command) {
            true
        } else {
            set_registry_named_value(APP_REGISTRY_KEY, EXTERNAL_ORIGINAL_COMMAND_VALUE, &original_command)
        }
    } else {
        true
    };

    if !saved_original {
        return false;
    }

    set_registry_named_value(APP_REGISTRY_KEY, EXTERNAL_ORIGINAL_COMMAND_SAVED_VALUE, "1")
}

fn set_external_link_command(command: &str) -> bool {
    if !save_external_link_original_command() {
        return false;
    }

    if !set_registry_default_value(EXTERNAL_LINK_COMMAND_KEY_HKCU, command) {
        return false;
    }

    let _ = set_registry_named_value(APP_REGISTRY_KEY, EXTERNAL_LAST_COMMAND_VALUE, command);

    true
}

fn restore_external_link_command() -> bool {
    let saved = query_registry_named_value(APP_REGISTRY_KEY, EXTERNAL_ORIGINAL_COMMAND_SAVED_VALUE).is_some();
    let original_command = query_registry_named_value(APP_REGISTRY_KEY, EXTERNAL_ORIGINAL_COMMAND_VALUE);

    let restored = if let Some(original_command) = original_command.as_ref() {
        set_registry_default_value(EXTERNAL_LINK_COMMAND_KEY_HKCU, original_command)
    } else if query_registry_default_value(EXTERNAL_LINK_COMMAND_KEY_HKCU).is_none() {
        true
    } else {
        delete_registry_key(EXTERNAL_LINK_COMMAND_KEY_HKCU)
    };

    if restored {
        if original_command.is_some() {
            delete_registry_named_value(APP_REGISTRY_KEY, EXTERNAL_ORIGINAL_COMMAND_VALUE);
        }

        if saved {
            delete_registry_named_value(APP_REGISTRY_KEY, EXTERNAL_ORIGINAL_COMMAND_SAVED_VALUE);
        }

        delete_registry_named_value(APP_REGISTRY_KEY, EXTERNAL_LAST_COMMAND_VALUE);
    }

    restored
}

fn apply_external_link_options(options: &str) -> ExternalLinkApplyResult {
    if options.trim().is_empty() {
        let restored = restore_external_link_command();

        return ExternalLinkApplyResult {
            state: if restored {
                ExternalLinkApplyState::Restored
            } else {
                ExternalLinkApplyState::Failed
            }
        };
    }

    let Some(command) = build_external_link_command(options) else {
        return ExternalLinkApplyResult {
            state: ExternalLinkApplyState::Failed
        };
    };

    ExternalLinkApplyResult {
        state: if set_external_link_command(&command) {
            ExternalLinkApplyState::Updated
        } else {
            ExternalLinkApplyState::Failed
        }
    }
}

pub fn get_current_user_external_link_command() -> Option<String> {
    query_registry_default_value(EXTERNAL_LINK_COMMAND_KEY_HKCU).filter(|value| !value.trim().is_empty())
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

        let len = buffer.iter().position(|value| *value == 0).unwrap_or(buffer.len());
        let arguments = String::from_utf16_lossy(&buffer[..len]);

        if arguments.trim().is_empty() {
            None
        } else {
            Some(arguments)
        }
    }
}

pub fn get_current_shortcut_arguments(selection: &ShortcutTargetSelection) -> String {
    for path in get_shortcut_paths(selection) {
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

        let arguments = merge_shortcut_arguments("", managed_options);

        shell_link.SetArguments(&HSTRING::from(arguments))?;
        persist_file.Save(&path_hstring, true)?;
    }

    Ok(())
}

fn apply_shortcut_path(result: &mut ApplyResult, shortcut_path: PathBuf, options: &str, show_missing: bool) {
    if !shortcut_path.exists() {
        if show_missing {
            result.details.push(ShortcutApplyDetail {
                path: shortcut_path,
                state: ShortcutApplyState::IgnoredMissing
            });
        }

        return;
    }

    result.found_shortcuts += 1;

    if update_shortcut_arguments(&shortcut_path, options).is_ok() {
        result.details.push(ShortcutApplyDetail {
            path: shortcut_path,
            state: ShortcutApplyState::Updated
        });
    } else {
        result.failed += 1;
        result.details.push(ShortcutApplyDetail {
            path: shortcut_path,
            state: ShortcutApplyState::Failed
        });
    }
}

pub fn apply_options(options: &str, selection: &ShortcutTargetSelection, apply_external_links: bool) -> ApplyResult {
    // Store per-shortcut results so the Info window can explain what happened.
    let mut result = ApplyResult {
        selected_shortcut_names: selected_shortcut_display_names(selection),
        found_shortcuts: 0,
        failed: 0,
        details: Vec::new(),
        external_links: None
    };

    for shortcut_path in get_shortcut_paths(selection) {
        apply_shortcut_path(&mut result, shortcut_path, options, true);
    }

    for shortcut_path in get_unselected_shortcut_paths(selection) {
        apply_shortcut_path(&mut result, shortcut_path, "", false);
    }

    let external_options = if apply_external_links {
        options
    } else {
        ""
    };

    result.external_links = Some(apply_external_link_options(external_options));

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const STANDALONE_EXAMPLE: &str = "--disable-extensions --force-dark-mode --mute-audio";
    const ENABLE_EXAMPLE: &str = "msForceNoRoundedCornerAndMargin,msUndersideButton,ParallelDownloading";
    const DISABLE_EXAMPLE: &str = "msShowSignInIndicator,msUndersideButton,MediaRouter";

    #[test]
    fn normalizes_plain_feature_list() {
        let input = " msForceNoRoundedCornerAndMargin, msUndersideButton ,, ParallelDownloading ";

        assert_eq!(normalize_feature_list(input), ENABLE_EXAMPLE);
    }

    #[test]
    fn normalizes_full_enable_switch() {
        let input = r#"--enable-features="msForceNoRoundedCornerAndMargin, msUndersideButton""#;

        assert_eq!(normalize_feature_list(input), "msForceNoRoundedCornerAndMargin,msUndersideButton");
    }

    #[test]
    fn builds_custom_options() {
        let options = get_custom_options_from_text(
            STANDALONE_EXAMPLE,
            "msForceNoRoundedCornerAndMargin, msUndersideButton, ParallelDownloading",
            r#"--disable-features="msShowSignInIndicator,msUndersideButton,MediaRouter""#
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
            "msForceNoRoundedCornerAndMargin,msUndersideButton"
        );

        assert_eq!(
            get_feature_list_from_arguments(&args, "enable-features"),
            "msForceNoRoundedCornerAndMargin,msUndersideButton"
        );
    }

    #[test]
    fn extracts_multiple_same_switches() {
        let args = concat!(
            r#"--enable-features=msForceNoRoundedCornerAndMargin "#,
            r#"--enable-features="msUndersideButton,ParallelDownloading""#
        );

        assert_eq!(get_feature_list_from_arguments(args, "enable-features"), ENABLE_EXAMPLE);
    }

    #[test]
    fn merge_replaces_existing_arguments() {
        let existing = concat!(
            r#"--profile-directory="Profile 1" --enable-features="msUndersideButton" "#,
            r#"--disable-extensions --app-id=abc"#
        );
        let managed = r#"--mute-audio --disable-features="MediaRouter""#;

        assert_eq!(merge_shortcut_arguments(existing, managed), managed);
    }

    #[test]
    fn restore_default_clears_arguments() {
        let existing = concat!(
            r#"--profile-directory="Default" "#,
            r#"--disable-features="msShowSignInIndicator,MediaRouter" "#,
            r#"--disable-extensions --app-id=abc"#
        );

        assert_eq!(merge_shortcut_arguments(existing, ""), "");
    }

    #[test]
    fn extracts_standalone_options_from_arguments() {
        let args = concat!(
            r#"--profile-directory="Default" --disable-extensions "#,
            r#"--enable-features="msForceNoRoundedCornerAndMargin" "#,
            r#"--force-dark-mode --mute-audio"#
        );

        assert_eq!(
            get_standalone_options_from_arguments(args),
            r#"--profile-directory="Default" --disable-extensions --force-dark-mode --mute-audio"#
        );
    }

    #[test]
    fn non_ascii_existing_arguments_do_not_affect_replacement() {
        let existing = r#"--profile-directory="Profilé 1" --enable-features="msUndersideButton""#;

        assert_eq!(
            merge_shortcut_arguments(existing, r#"--disable-features="MediaRouter""#),
            r#"--disable-features="MediaRouter""#
        );
    }
}
