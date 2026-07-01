#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod shortcut;

use std::cell::RefCell;
use std::fmt::Write;
use std::path::Path;
use std::rc::Rc;

use native_windows_gui as nwg;
use windows::Win32::Foundation::{
    HWND, LPARAM, POINT, RECT, WPARAM
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    DeleteMenu, DrawMenuBar, GetCursorPos, GetSystemMenu, GetWindowLongPtrW, SendMessageW,
    SetClassLongPtrW, SetWindowLongPtrW, SetWindowPos, GCLP_HICON, GCLP_HICONSM,
    GWL_EXSTYLE, ICON_BIG, ICON_SMALL, MF_BYCOMMAND, SC_MAXIMIZE, SC_MINIMIZE,
    SC_RESTORE, SC_SIZE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, WM_SETICON, WS_EX_DLGMODALFRAME
};

const APP_TITLE: &str = "Edge Shortcut Tool";
const CUSTOM_WINDOW_CENTER_Y_OFFSET: i32 = -3;

const OLD_FEATURE: &str = "msFeatureGroupNewLookAndFeelHoldout";
const NEW_FEATURE: &str = "msForceNoRoundedCornerAndMargin";
const SIGN_IN_FEATURE: &str = "msShowSignInIndicator";
const SIDEBAR_FEATURE: &str = "msHubAppsSidebarRetirement";
const DISABLE_EXTENSIONS_OPTION: &str = "--disable-extensions";

const COLOR_CONTROL: [u8; 3] = [240, 240, 240];
const COLOR_WINDOW: [u8; 3] = [255, 255, 255];
const COLOR_GROUP_LINE: [u8; 3] = [198, 198, 198];
const BLACK: [u8; 3] = [0, 0, 0];
const NOTE_RED: [u8; 3] = [225, 0, 0];
const LINK_BLUE: [u8; 3] = [0, 0, 225];
const EM_SETRECT_MESSAGE: u32 = 0x00B3;
const HELP_TEXT: &str = concat!(
    "Old workaround\r\n",
    "Uses msFeatureGroupNewLookAndFeelHoldout.\r\n",
    "This is the older/broader method for removing rounded corner look.\r\n",
    "\r\n",
    "New workaround\r\n",
    "Uses msForceNoRoundedCornerAndMargin.\r\n",
    "This is the newer/more direct method for removing rounded corners and margins.\r\n",
    "\r\n",
    "Restore default\r\n",
    "Clears all command-line flags managed by this tool and restores normal launch behavior.\r\n",
    "\r\n",
    "Hide sign-in red dot\r\n",
    "When checked, this also disables msShowSignInIndicator.\r\n",
    "This hides the red sign-in reminder dot on profile icon.\r\n",
    "\r\n",
    "Restore sidebar\r\n",
    "When checked, this also disables msHubAppsSidebarRetirement.\r\n",
    "This attempts to restore the classic sidebar app list.\r\n",
    "\r\n",
    "Disable extensions\r\n",
    "When checked, this also adds --disable-extensions.\r\n",
    "This starts Edge without loading browser extensions.\r\n",
    "\r\n",
    "Advanced\r\n",
    "Choose which shortcut names are updated.\r\n",
    "You can also apply flags when Edge is opened by another app such as Discord or Teams.\r\n",
    "\r\n",
    "Custom\r\n",
    "Standalone example: --disable-extensions --force-dark-mode --mute-audio\r\n",
    "Enable example: msForceNoRoundedCornerAndMargin,msUndersideButton,ParallelDownloading\r\n",
    "Disable example: msShowSignInIndicator,msHubAppsSidebarRetirement,MediaRouter\r\n",
    "\r\n",
    "Info\r\n",
    "Displays executable candidates and selected shortcut locations.\r\n",
    "For reference and diagnostic purposes only.\r\n",
    "\r\n",
    "Reminder\r\n",
    "Disable Startup Boost in Edge first. Copy and paste this into Edge address bar: ",
    "edge://settings/system/manageSystem"
);

#[derive(Default)]
struct App {
    window: nwg::Window,

    button_shortcuts: nwg::Button,
    button_custom: nwg::Button,
    button_help: nwg::Button,

    preset_panel: nwg::Frame,
    preset_line_top_left: nwg::RichLabel,
    preset_line_top_right: nwg::RichLabel,
    preset_line_left: nwg::RichLabel,
    preset_line_right: nwg::RichLabel,
    preset_line_bottom: nwg::RichLabel,
    preset_title: nwg::RichLabel,
    button_old: nwg::Button,
    button_new: nwg::Button,
    button_default: nwg::Button,

    check_sign_in_indicator: nwg::CheckBox,
    check_restore_sidebar: nwg::CheckBox,
    check_disable_extensions: nwg::CheckBox,

    status_box: nwg::RichTextBox,
    button_info: nwg::Button,
    button_exit: nwg::Button,

    shortcuts_window: nwg::Window,
    shortcut_target_line_top_left: nwg::RichLabel,
    shortcut_target_line_top_right: nwg::RichLabel,
    shortcut_target_line_left: nwg::RichLabel,
    shortcut_target_line_right: nwg::RichLabel,
    shortcut_target_line_bottom: nwg::RichLabel,
    label_shortcuts_title: nwg::RichLabel,
    label_shortcuts_description: nwg::Label,
    external_links_line_top_left: nwg::RichLabel,
    external_links_line_top_right: nwg::RichLabel,
    external_links_line_left: nwg::RichLabel,
    external_links_line_right: nwg::RichLabel,
    external_links_line_bottom: nwg::RichLabel,
    label_external_links_title: nwg::RichLabel,
    label_external_links_description: nwg::Label,
    check_shortcut_stable: nwg::CheckBox,
    check_shortcut_beta: nwg::CheckBox,
    check_shortcut_dev: nwg::CheckBox,
    check_shortcut_canary: nwg::CheckBox,
    check_external_links: nwg::CheckBox,
    button_shortcuts_ok: nwg::Button,
    button_shortcuts_cancel: nwg::Button,

    custom_window: nwg::Window,
    label_standalone_flags: nwg::Label,
    text_standalone_flags: nwg::TextInput,
    label_enable_features: nwg::Label,
    text_enable_features: nwg::TextInput,
    label_disable_features: nwg::Label,
    text_disable_features: nwg::TextInput,
    button_apply_custom: nwg::Button,
    button_close_custom: nwg::Button,

    help_window: nwg::Window,
    help_text: nwg::RichTextBox,
    button_help_ok: nwg::Button,

    status_window: nwg::Window,
    status_edge_header: nwg::RichLabel,
    status_edge_text: nwg::RichTextBox,
    status_shortcut_header: nwg::RichLabel,
    status_shortcut_text: nwg::RichTextBox,
    button_status_ok: nwg::Button,

    ui_font: nwg::Font,
    info_font: nwg::Font,
    detail_font: nwg::Font,
    shortcut_button_font: nwg::Font,
    tooltip: nwg::Tooltip,
    shortcut_selection: RefCell<shortcut::ShortcutTargetSelection>,
    apply_external_links: RefCell<bool>,
    last_apply_result: RefCell<Option<shortcut::ApplyResult>>
}

impl App {
    fn build() -> Result<Self, nwg::NwgError> {
        let mut app = App::default();

        app.build_fonts()?;
        app.build_main_window()?;
        app.build_shortcuts_window()?;
        app.build_custom_window()?;
        app.build_help_window()?;
        app.load_settings();
        app.build_status_window()?;
        app.build_tooltips()?;
        app.style_help_text();
        app.harden_window_chrome();
        app.center_main_window();

        app.window.set_visible(true);
        app.button_help.set_focus();

        Ok(app)
    }

    fn build_fonts(&mut self) -> Result<(), nwg::NwgError> {
        nwg::Font::builder()
            .family("Segoe UI")
            .size_absolute(12)
            .build(&mut self.ui_font)?;

        nwg::Font::builder()
            .family("Segoe UI")
            .size_absolute(12)
            .weight(700)
            .build(&mut self.info_font)?;

        nwg::Font::builder()
            .family("Segoe UI")
            .size_absolute(12)
            .build(&mut self.detail_font)?;

        nwg::Font::builder()
            .family("Segoe UI")
            .size_absolute(13)
            .build(&mut self.shortcut_button_font)?;

        Ok(())
    }

    fn build_main_window(&mut self) -> Result<(), nwg::NwgError> {
        const WINDOW_W: i32 = 312;

        const MARGIN: i32 = 14;
        const TOP_BUTTON_Y: i32 = 12;
        const TOP_BUTTON_W: i32 = 27;
        const TOP_BUTTON_H: i32 = 25;
        const TOP_BUTTON_GAP: i32 = 5;
        const RIGHT_EDGE: i32 = WINDOW_W - MARGIN;
        const HELP_X: i32 = RIGHT_EDGE - TOP_BUTTON_W;
        const CUSTOM_X: i32 = HELP_X - TOP_BUTTON_GAP - TOP_BUTTON_W;
        const SHORTCUTS_X: i32 = CUSTOM_X - TOP_BUTTON_GAP - TOP_BUTTON_W;

        const PRESET_X: i32 = MARGIN;
        const PRESET_Y: i32 = 42;
        const PRESET_W: i32 = WINDOW_W - (MARGIN * 2);
        const PRESET_H: i32 = 140;
        const PRESET_RIGHT_X: i32 = PRESET_X + PRESET_W;
        const PRESET_BOTTOM_Y: i32 = PRESET_Y + PRESET_H - 1;

        const TITLE_X: i32 = 24;
        const TITLE_Y: i32 = 36;
        const TITLE_W: i32 = 109;
        const TITLE_H: i32 = 20;
        const TITLE_RIGHT_X: i32 = TITLE_X + TITLE_W;
        const PRESET_LINE_TOP_LEFT_W: i32 = TITLE_X - PRESET_X - 1;
        const PRESET_LINE_TOP_RIGHT_W: i32 = PRESET_RIGHT_X - TITLE_RIGHT_X;

        const PRESET_BUTTON_X: i32 = 16;
        const PRESET_BUTTON_Y: i32 = 19;
        const PRESET_BUTTON_W: i32 = PRESET_W - (PRESET_BUTTON_X * 2);
        const PRESET_BUTTON_H: i32 = 30;
        const PRESET_BUTTON_GAP: i32 = 6;
        const OLD_BUTTON_Y: i32 = PRESET_BUTTON_Y;
        const NEW_BUTTON_Y: i32 = OLD_BUTTON_Y + PRESET_BUTTON_H + PRESET_BUTTON_GAP;
        const DEFAULT_BUTTON_Y: i32 = NEW_BUTTON_Y + PRESET_BUTTON_H + PRESET_BUTTON_GAP;

        const CHECK_X: i32 = 16;
        const CHECK_Y: i32 = PRESET_Y + PRESET_H + 8;
        const CHECK_W: i32 = 250;
        const CHECK_H: i32 = 22;
        const CHECK_GAP: i32 = 2;
        const SIGN_IN_CHECK_Y: i32 = CHECK_Y;
        const SIDEBAR_CHECK_Y: i32 = SIGN_IN_CHECK_Y + CHECK_H + CHECK_GAP;
        const EXTENSIONS_CHECK_Y: i32 = SIDEBAR_CHECK_Y + CHECK_H + CHECK_GAP;

        const ACTION_Y: i32 = EXTENSIONS_CHECK_Y + CHECK_H + 12;
        const ACTION_H: i32 = 26;
        const ACTION_GAP: i32 = 5;
        const EXIT_W: i32 = 73;
        const INFO_W: i32 = 27;
        const EXIT_X: i32 = RIGHT_EDGE - EXIT_W;
        const INFO_X: i32 = EXIT_X - ACTION_GAP - INFO_W;
        const STATUS_X: i32 = MARGIN;
        const STATUS_W: i32 = INFO_X - ACTION_GAP - STATUS_X;
        const WINDOW_H: i32 = ACTION_Y + ACTION_H + MARGIN;

        nwg::Window::builder()
            .flags(nwg::WindowFlags::WINDOW)
            .size((WINDOW_W, WINDOW_H))
            .position((400, 300))
            .title(APP_TITLE)
            .build(&mut self.window)?;

        nwg::Button::builder()
            .text("\u{25A6}")
            .position((SHORTCUTS_X, TOP_BUTTON_Y))
            .size((TOP_BUTTON_W, TOP_BUTTON_H))
            .font(Some(&self.shortcut_button_font))
            .parent(&self.window)
            .build(&mut self.button_shortcuts)?;

        nwg::Button::builder()
            .text("...")
            .position((CUSTOM_X, TOP_BUTTON_Y))
            .size((TOP_BUTTON_W, TOP_BUTTON_H))
            .font(Some(&self.ui_font))
            .parent(&self.window)
            .build(&mut self.button_custom)?;

        nwg::Button::builder()
            .text("?")
            .position((HELP_X, TOP_BUTTON_Y))
            .size((TOP_BUTTON_W, TOP_BUTTON_H))
            .font(Some(&self.ui_font))
            .parent(&self.window)
            .build(&mut self.button_help)?;

        // Draw the preset border manually so the group line stays soft and consistent.
        nwg::Frame::builder()
            .flags(nwg::FrameFlags::VISIBLE)
            .position((PRESET_X, PRESET_Y))
            .size((PRESET_W, PRESET_H))
            .parent(&self.window)
            .build(&mut self.preset_panel)?;

        nwg::RichLabel::builder()
            .text("")
            .position((PRESET_X, PRESET_Y))
            .size((PRESET_LINE_TOP_LEFT_W, 1))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.window)
            .build(&mut self.preset_line_top_left)?;

        nwg::RichLabel::builder()
            .text("")
            .position((TITLE_RIGHT_X, PRESET_Y))
            .size((PRESET_LINE_TOP_RIGHT_W, 1))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.window)
            .build(&mut self.preset_line_top_right)?;

        nwg::RichLabel::builder()
            .text("")
            .position((PRESET_X, PRESET_Y))
            .size((1, PRESET_H))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.window)
            .build(&mut self.preset_line_left)?;

        nwg::RichLabel::builder()
            .text("")
            .position((PRESET_RIGHT_X - 1, PRESET_Y))
            .size((1, PRESET_H))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.window)
            .build(&mut self.preset_line_right)?;

        nwg::RichLabel::builder()
            .text("")
            .position((PRESET_X, PRESET_BOTTOM_Y))
            .size((PRESET_W, 1))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.window)
            .build(&mut self.preset_line_bottom)?;

        nwg::RichLabel::builder()
            .text("Fix rounded corners")
            .position((TITLE_X, TITLE_Y))
            .size((TITLE_W, TITLE_H))
            .font(Some(&self.ui_font))
            .background_color(Some(COLOR_CONTROL))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.window)
            .build(&mut self.preset_title)?;

        nwg::Button::builder()
            .text("Old workaround")
            .position((PRESET_BUTTON_X, OLD_BUTTON_Y))
            .size((PRESET_BUTTON_W, PRESET_BUTTON_H))
            .parent(&self.preset_panel)
            .build(&mut self.button_old)?;

        nwg::Button::builder()
            .text("New workaround")
            .position((PRESET_BUTTON_X, NEW_BUTTON_Y))
            .size((PRESET_BUTTON_W, PRESET_BUTTON_H))
            .parent(&self.preset_panel)
            .build(&mut self.button_new)?;

        nwg::Button::builder()
            .text("Restore default")
            .position((PRESET_BUTTON_X, DEFAULT_BUTTON_Y))
            .size((PRESET_BUTTON_W, PRESET_BUTTON_H))
            .parent(&self.preset_panel)
            .build(&mut self.button_default)?;

        nwg::CheckBox::builder()
            .text("Hide sign-in red dot")
            .position((CHECK_X, SIGN_IN_CHECK_Y))
            .size((CHECK_W, CHECK_H))
            .parent(&self.window)
            .build(&mut self.check_sign_in_indicator)?;

        self.check_sign_in_indicator.set_check_state(nwg::CheckBoxState::Checked);

        nwg::CheckBox::builder()
            .text("Restore sidebar")
            .position((CHECK_X, SIDEBAR_CHECK_Y))
            .size((CHECK_W, CHECK_H))
            .parent(&self.window)
            .build(&mut self.check_restore_sidebar)?;

        nwg::CheckBox::builder()
            .text("Disable extensions")
            .position((CHECK_X, EXTENSIONS_CHECK_Y))
            .size((CHECK_W, CHECK_H))
            .parent(&self.window)
            .build(&mut self.check_disable_extensions)?;

        // RichTextBox gives the status field the same sunken Win32 look as the dialogs.
        nwg::RichTextBox::builder()
            .position((STATUS_X, ACTION_Y))
            .size((STATUS_W, ACTION_H))
            .font(Some(&self.ui_font))
            .readonly(false)
            .flags(nwg::RichTextBoxFlags::VISIBLE | nwg::RichTextBoxFlags::SAVE_SELECTION)
            .parent(&self.window)
            .build(&mut self.status_box)?;

        self.status_box.set_text("Ready");
        self.status_box.set_background_color(COLOR_CONTROL);
        self.status_box.set_readonly(true);

        nwg::Button::builder()
            .text("i")
            .position((INFO_X, ACTION_Y))
            .size((INFO_W, ACTION_H))
            .font(Some(&self.ui_font))
            .parent(&self.window)
            .build(&mut self.button_info)?;

        nwg::Button::builder()
            .text("Exit")
            .position((EXIT_X, ACTION_Y))
            .size((EXIT_W, ACTION_H))
            .parent(&self.window)
            .build(&mut self.button_exit)?;

        Ok(())
    }

    fn build_shortcuts_window(&mut self) -> Result<(), nwg::NwgError> {
        const WINDOW_W: i32 = 400;

        const MARGIN_X: i32 = 16;
        const SECTION_X: i32 = MARGIN_X;
        const SECTION_W: i32 = WINDOW_W - (MARGIN_X * 2);
        const SECTION_RIGHT_X: i32 = SECTION_X + SECTION_W;
        const SECTION_TITLE_X: i32 = SECTION_X + 10;
        const SECTION_TITLE_Y_OFFSET: i32 = -7;
        const SECTION_TITLE_H: i32 = 20;
        const SECTION_CONTENT_X: i32 = SECTION_X + 12;
        const SECTION_CONTENT_W: i32 = SECTION_W - 24;

        const TARGET_SECTION_Y: i32 = 20;
        const TARGET_SECTION_H: i32 = 158;
        const TARGET_SECTION_BOTTOM_Y: i32 = TARGET_SECTION_Y + TARGET_SECTION_H - 1;
        const TARGET_TITLE_Y: i32 = TARGET_SECTION_Y + SECTION_TITLE_Y_OFFSET;
        const TARGET_TITLE_W: i32 = 88;
        const TARGET_TITLE_RIGHT_X: i32 = SECTION_TITLE_X + TARGET_TITLE_W;
        const TARGET_LINE_TOP_LEFT_W: i32 = SECTION_TITLE_X - SECTION_X - 1;
        const TARGET_LINE_TOP_RIGHT_W: i32 = SECTION_RIGHT_X - TARGET_TITLE_RIGHT_X;

        const DESCRIPTION_H: i32 = 20;
        const TARGET_DESCRIPTION_Y: i32 = TARGET_SECTION_Y + 22;

        const CHECK_X: i32 = SECTION_CONTENT_X;
        const CHECK_W: i32 = SECTION_CONTENT_W;
        const CHECK_H: i32 = 22;
        const CHECK_GAP: i32 = 4;
        const CHECK_START_Y: i32 = TARGET_DESCRIPTION_Y + DESCRIPTION_H + 6;
        const STABLE_Y: i32 = CHECK_START_Y;
        const BETA_Y: i32 = STABLE_Y + CHECK_H + CHECK_GAP;
        const DEV_Y: i32 = BETA_Y + CHECK_H + CHECK_GAP;
        const CANARY_Y: i32 = DEV_Y + CHECK_H + CHECK_GAP;

        const EXTERNAL_SECTION_GAP: i32 = 12;
        const EXTERNAL_SECTION_Y: i32 = TARGET_SECTION_Y + TARGET_SECTION_H + EXTERNAL_SECTION_GAP;
        const EXTERNAL_SECTION_H: i32 = 96;
        const EXTERNAL_SECTION_BOTTOM_Y: i32 = EXTERNAL_SECTION_Y + EXTERNAL_SECTION_H - 1;
        const EXTERNAL_TITLE_Y: i32 = EXTERNAL_SECTION_Y + SECTION_TITLE_Y_OFFSET;
        const EXTERNAL_TITLE_W: i32 = 73;
        const EXTERNAL_TITLE_RIGHT_X: i32 = SECTION_TITLE_X + EXTERNAL_TITLE_W;
        const EXTERNAL_LINE_TOP_LEFT_W: i32 = SECTION_TITLE_X - SECTION_X - 1;
        const EXTERNAL_LINE_TOP_RIGHT_W: i32 = SECTION_RIGHT_X - EXTERNAL_TITLE_RIGHT_X;
        const EXTERNAL_DESCRIPTION_Y: i32 = EXTERNAL_SECTION_Y + 22;
        const EXTERNAL_DESCRIPTION_H: i32 = 40;
        const EXTERNAL_CHECK_Y: i32 = EXTERNAL_DESCRIPTION_Y + EXTERNAL_DESCRIPTION_H + 2;

        const BUTTON_W: i32 = 75;
        const BUTTON_H: i32 = 26;
        const BUTTON_GAP: i32 = 8;
        const BUTTON_SECTION_GAP: i32 = 14;
        const BUTTON_Y: i32 = EXTERNAL_SECTION_Y + EXTERNAL_SECTION_H + BUTTON_SECTION_GAP;
        const BUTTON_CANCEL_X: i32 = WINDOW_W - MARGIN_X - BUTTON_W;
        const BUTTON_OK_X: i32 = BUTTON_CANCEL_X - BUTTON_GAP - BUTTON_W;
        const WINDOW_H: i32 = BUTTON_Y + BUTTON_H + BUTTON_SECTION_GAP;

        nwg::Window::builder()
            .flags(nwg::WindowFlags::WINDOW)
            .size((WINDOW_W, WINDOW_H))
            .position((430, 330))
            .title("Advanced")
            .build(&mut self.shortcuts_window)?;

        nwg::RichLabel::builder()
            .text("")
            .position((SECTION_X, TARGET_SECTION_Y))
            .size((TARGET_LINE_TOP_LEFT_W, 1))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.shortcuts_window)
            .build(&mut self.shortcut_target_line_top_left)?;

        nwg::RichLabel::builder()
            .text("")
            .position((TARGET_TITLE_RIGHT_X, TARGET_SECTION_Y))
            .size((TARGET_LINE_TOP_RIGHT_W, 1))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.shortcuts_window)
            .build(&mut self.shortcut_target_line_top_right)?;

        nwg::RichLabel::builder()
            .text("")
            .position((SECTION_X, TARGET_SECTION_Y))
            .size((1, TARGET_SECTION_H))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.shortcuts_window)
            .build(&mut self.shortcut_target_line_left)?;

        nwg::RichLabel::builder()
            .text("")
            .position((SECTION_RIGHT_X - 1, TARGET_SECTION_Y))
            .size((1, TARGET_SECTION_H))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.shortcuts_window)
            .build(&mut self.shortcut_target_line_right)?;

        nwg::RichLabel::builder()
            .text("")
            .position((SECTION_X, TARGET_SECTION_BOTTOM_Y))
            .size((SECTION_W, 1))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.shortcuts_window)
            .build(&mut self.shortcut_target_line_bottom)?;

        nwg::RichLabel::builder()
            .text("Shortcut targets")
            .position((SECTION_TITLE_X, TARGET_TITLE_Y))
            .size((TARGET_TITLE_W, SECTION_TITLE_H))
            .font(Some(&self.ui_font))
            .background_color(Some(COLOR_CONTROL))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.shortcuts_window)
            .build(&mut self.label_shortcuts_title)?;

        nwg::Label::builder()
            .text("Choose which shortcut names are updated.")
            .position((SECTION_CONTENT_X, TARGET_DESCRIPTION_Y))
            .size((SECTION_CONTENT_W, DESCRIPTION_H))
            .parent(&self.shortcuts_window)
            .build(&mut self.label_shortcuts_description)?;

        nwg::CheckBox::builder()
            .text("Microsoft Edge")
            .position((CHECK_X, STABLE_Y))
            .size((CHECK_W, CHECK_H))
            .parent(&self.shortcuts_window)
            .build(&mut self.check_shortcut_stable)?;

        nwg::CheckBox::builder()
            .text("Microsoft Edge Beta")
            .position((CHECK_X, BETA_Y))
            .size((CHECK_W, CHECK_H))
            .parent(&self.shortcuts_window)
            .build(&mut self.check_shortcut_beta)?;

        nwg::CheckBox::builder()
            .text("Microsoft Edge Dev")
            .position((CHECK_X, DEV_Y))
            .size((CHECK_W, CHECK_H))
            .parent(&self.shortcuts_window)
            .build(&mut self.check_shortcut_dev)?;

        nwg::CheckBox::builder()
            .text("Microsoft Edge Canary")
            .position((CHECK_X, CANARY_Y))
            .size((CHECK_W, CHECK_H))
            .parent(&self.shortcuts_window)
            .build(&mut self.check_shortcut_canary)?;

        nwg::RichLabel::builder()
            .text("")
            .position((SECTION_X, EXTERNAL_SECTION_Y))
            .size((EXTERNAL_LINE_TOP_LEFT_W, 1))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.shortcuts_window)
            .build(&mut self.external_links_line_top_left)?;

        nwg::RichLabel::builder()
            .text("")
            .position((EXTERNAL_TITLE_RIGHT_X, EXTERNAL_SECTION_Y))
            .size((EXTERNAL_LINE_TOP_RIGHT_W, 1))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.shortcuts_window)
            .build(&mut self.external_links_line_top_right)?;

        nwg::RichLabel::builder()
            .text("")
            .position((SECTION_X, EXTERNAL_SECTION_Y))
            .size((1, EXTERNAL_SECTION_H))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.shortcuts_window)
            .build(&mut self.external_links_line_left)?;

        nwg::RichLabel::builder()
            .text("")
            .position((SECTION_RIGHT_X - 1, EXTERNAL_SECTION_Y))
            .size((1, EXTERNAL_SECTION_H))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.shortcuts_window)
            .build(&mut self.external_links_line_right)?;

        nwg::RichLabel::builder()
            .text("")
            .position((SECTION_X, EXTERNAL_SECTION_BOTTOM_Y))
            .size((SECTION_W, 1))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.shortcuts_window)
            .build(&mut self.external_links_line_bottom)?;

        nwg::RichLabel::builder()
            .text("External links")
            .position((SECTION_TITLE_X, EXTERNAL_TITLE_Y))
            .size((EXTERNAL_TITLE_W, SECTION_TITLE_H))
            .font(Some(&self.ui_font))
            .background_color(Some(COLOR_CONTROL))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.shortcuts_window)
            .build(&mut self.label_external_links_title)?;

        nwg::Label::builder()
            .text("Use the same flags when Edge is opened by another app.\r\nSuch as Discord, Teams, Terminal.")
            .position((SECTION_CONTENT_X, EXTERNAL_DESCRIPTION_Y))
            .size((SECTION_CONTENT_W, EXTERNAL_DESCRIPTION_H))
            .parent(&self.shortcuts_window)
            .build(&mut self.label_external_links_description)?;

        nwg::CheckBox::builder()
            .text("Apply to external links")
            .position((CHECK_X, EXTERNAL_CHECK_Y))
            .size((CHECK_W, CHECK_H))
            .parent(&self.shortcuts_window)
            .build(&mut self.check_external_links)?;

        nwg::Button::builder()
            .text("OK")
            .position((BUTTON_OK_X, BUTTON_Y))
            .size((BUTTON_W, BUTTON_H))
            .parent(&self.shortcuts_window)
            .build(&mut self.button_shortcuts_ok)?;

        nwg::Button::builder()
            .text("Cancel")
            .position((BUTTON_CANCEL_X, BUTTON_Y))
            .size((BUTTON_W, BUTTON_H))
            .parent(&self.shortcuts_window)
            .build(&mut self.button_shortcuts_cancel)?;

        self.shortcuts_window.set_visible(false);

        Ok(())
    }

    fn build_custom_window(&mut self) -> Result<(), nwg::NwgError> {
        const WINDOW_W: i32 = 470;

        const MARGIN_X: i32 = 16;
        const LABEL_W: i32 = WINDOW_W - (MARGIN_X * 2);
        const LABEL_H: i32 = 18;
        const TEXT_W: i32 = LABEL_W;
        const TEXT_H: i32 = 24;

        const TOP_MARGIN: i32 = 18;
        const LABEL_TEXT_GAP: i32 = 1;
        const FIELD_GAP: i32 = 6;
        const ACTION_GAP: i32 = 14;
        const BOTTOM_MARGIN: i32 = 14;

        const STANDALONE_LABEL_Y: i32 = TOP_MARGIN;
        const STANDALONE_TEXT_Y: i32 = STANDALONE_LABEL_Y + LABEL_H + LABEL_TEXT_GAP;
        const ENABLE_LABEL_Y: i32 = STANDALONE_TEXT_Y + TEXT_H + FIELD_GAP;
        const ENABLE_TEXT_Y: i32 = ENABLE_LABEL_Y + LABEL_H + LABEL_TEXT_GAP;
        const DISABLE_LABEL_Y: i32 = ENABLE_TEXT_Y + TEXT_H + FIELD_GAP;
        const DISABLE_TEXT_Y: i32 = DISABLE_LABEL_Y + LABEL_H + LABEL_TEXT_GAP;

        const BUTTON_W: i32 = 75;
        const BUTTON_H: i32 = 26;
        const BUTTON_GAP: i32 = 8;
        const BUTTON_Y: i32 = DISABLE_TEXT_Y + TEXT_H + ACTION_GAP;
        const BUTTON_CLOSE_X: i32 = WINDOW_W - MARGIN_X - BUTTON_W;
        const BUTTON_APPLY_X: i32 = BUTTON_CLOSE_X - BUTTON_GAP - BUTTON_W;
        const WINDOW_H: i32 = BUTTON_Y + BUTTON_H + BOTTOM_MARGIN;

        nwg::Window::builder()
            .flags(nwg::WindowFlags::WINDOW)
            .size((WINDOW_W, WINDOW_H))
            .position((430, 340))
            .title("Custom")
            .build(&mut self.custom_window)?;

        nwg::Label::builder()
            .text("Standalone")
            .position((MARGIN_X, STANDALONE_LABEL_Y))
            .size((LABEL_W, LABEL_H))
            .parent(&self.custom_window)
            .build(&mut self.label_standalone_flags)?;

        nwg::TextInput::builder()
            .position((MARGIN_X, STANDALONE_TEXT_Y))
            .size((TEXT_W, TEXT_H))
            .parent(&self.custom_window)
            .build(&mut self.text_standalone_flags)?;

        nwg::Label::builder()
            .text("Enable features:")
            .position((MARGIN_X, ENABLE_LABEL_Y))
            .size((LABEL_W, LABEL_H))
            .parent(&self.custom_window)
            .build(&mut self.label_enable_features)?;

        nwg::TextInput::builder()
            .position((MARGIN_X, ENABLE_TEXT_Y))
            .size((TEXT_W, TEXT_H))
            .parent(&self.custom_window)
            .build(&mut self.text_enable_features)?;

        nwg::Label::builder()
            .text("Disable features:")
            .position((MARGIN_X, DISABLE_LABEL_Y))
            .size((LABEL_W, LABEL_H))
            .parent(&self.custom_window)
            .build(&mut self.label_disable_features)?;

        nwg::TextInput::builder()
            .position((MARGIN_X, DISABLE_TEXT_Y))
            .size((TEXT_W, TEXT_H))
            .parent(&self.custom_window)
            .build(&mut self.text_disable_features)?;

        nwg::Button::builder()
            .text("Apply")
            .position((BUTTON_APPLY_X, BUTTON_Y))
            .size((BUTTON_W, BUTTON_H))
            .parent(&self.custom_window)
            .build(&mut self.button_apply_custom)?;

        nwg::Button::builder()
            .text("Close")
            .position((BUTTON_CLOSE_X, BUTTON_Y))
            .size((BUTTON_W, BUTTON_H))
            .parent(&self.custom_window)
            .build(&mut self.button_close_custom)?;

        self.custom_window.set_visible(false);

        Ok(())
    }

    fn build_help_window(&mut self) -> Result<(), nwg::NwgError> {
        const WINDOW_W: i32 = 600;

        const MARGIN: i32 = 14;
        const BUTTON_SECTION_GAP: i32 = 14;
        const BUTTON_W: i32 = 75;
        const BUTTON_H: i32 = 26;

        const TEXT_X: i32 = MARGIN;
        const TEXT_Y: i32 = MARGIN;
        const TEXT_W: i32 = WINDOW_W - (MARGIN * 2);
        const TEXT_H: i32 = 610;
        const BUTTON_X: i32 = TEXT_X + TEXT_W - BUTTON_W;
        const BUTTON_Y: i32 = TEXT_Y + TEXT_H + BUTTON_SECTION_GAP;
        const WINDOW_H: i32 = BUTTON_Y + BUTTON_H + BUTTON_SECTION_GAP;

        nwg::Window::builder()
            .flags(nwg::WindowFlags::WINDOW)
            .size((WINDOW_W, WINDOW_H))
            .position((450, 280))
            .title("Help")
            .build(&mut self.help_window)?;

        nwg::RichTextBox::builder()
            .position((TEXT_X, TEXT_Y))
            .size((TEXT_W, TEXT_H))
            .font(Some(&self.ui_font))
            .readonly(false)
            .flags(nwg::RichTextBoxFlags::VISIBLE | nwg::RichTextBoxFlags::SAVE_SELECTION)
            .parent(&self.help_window)
            .build(&mut self.help_text)?;

        self.help_text.set_background_color(COLOR_WINDOW);

        nwg::Button::builder()
            .text("OK")
            .position((BUTTON_X, BUTTON_Y))
            .size((BUTTON_W, BUTTON_H))
            .parent(&self.help_window)
            .build(&mut self.button_help_ok)?;

        self.help_window.set_visible(false);

        Ok(())
    }

    fn build_status_window(&mut self) -> Result<(), nwg::NwgError> {
        const WINDOW_W: i32 = 930;

        const MARGIN: i32 = 16;
        const BUTTON_SECTION_GAP: i32 = 14;
        const SECTION_GAP: i32 = 16;
        const HEADER_H: i32 = 20;
        const HEADER_LINE_HEIGHT: i32 = 18;
        const HEADER_TEXT_GAP: i32 = 4;
        const CONTENT_W: i32 = WINDOW_W - (MARGIN * 2);

        const EDGE_HEADER_Y: i32 = MARGIN;
        const EDGE_TEXT_Y: i32 = EDGE_HEADER_Y + HEADER_H + HEADER_TEXT_GAP;
        const EDGE_TEXT_H: i32 = 232;

        const SHORTCUT_HEADER_Y: i32 = EDGE_TEXT_Y + EDGE_TEXT_H + SECTION_GAP;
        const SHORTCUT_TEXT_Y: i32 = SHORTCUT_HEADER_Y + HEADER_H + HEADER_TEXT_GAP;
        const SHORTCUT_TEXT_H: i32 = 382;

        const BUTTON_W: i32 = 75;
        const BUTTON_H: i32 = 26;
        const BUTTON_X: i32 = MARGIN + CONTENT_W - BUTTON_W;
        const BUTTON_Y: i32 = SHORTCUT_TEXT_Y + SHORTCUT_TEXT_H + BUTTON_SECTION_GAP;
        const WINDOW_H: i32 = BUTTON_Y + BUTTON_H + BUTTON_SECTION_GAP;

        // Keep diagnostic sections scrollable when future output becomes longer.
        let info_text_flags = nwg::RichTextBoxFlags::VISIBLE
            | nwg::RichTextBoxFlags::SAVE_SELECTION
            | nwg::RichTextBoxFlags::VSCROLL
            | nwg::RichTextBoxFlags::AUTOVSCROLL;

        nwg::Window::builder()
            .flags(nwg::WindowFlags::WINDOW)
            .size((WINDOW_W, WINDOW_H))
            .position((460, 290))
            .title("Info")
            .build(&mut self.status_window)?;

        // Disabled RichLabel avoids cursor selection while keeping the simple header style.
        nwg::RichLabel::builder()
            .text("EXECUTABLES")
            .position((MARGIN, EDGE_HEADER_Y))
            .size((CONTENT_W, HEADER_H))
            .font(Some(&self.info_font))
            .background_color(Some(COLOR_GROUP_LINE))
            .line_height(Some(HEADER_LINE_HEIGHT))
            .flags(nwg::RichLabelFlags::VISIBLE | nwg::RichLabelFlags::DISABLED)
            .parent(&self.status_window)
            .build(&mut self.status_edge_header)?;

        nwg::RichTextBox::builder()
            .position((MARGIN, EDGE_TEXT_Y))
            .size((CONTENT_W, EDGE_TEXT_H))
            .font(Some(&self.detail_font))
            .readonly(false)
            .flags(info_text_flags)
            .parent(&self.status_window)
            .build(&mut self.status_edge_text)?;

        self.status_edge_text.set_background_color(COLOR_WINDOW);

        nwg::RichLabel::builder()
            .text("SHORTCUTS")
            .position((MARGIN, SHORTCUT_HEADER_Y))
            .size((CONTENT_W, HEADER_H))
            .font(Some(&self.info_font))
            .background_color(Some(COLOR_GROUP_LINE))
            .line_height(Some(HEADER_LINE_HEIGHT))
            .flags(nwg::RichLabelFlags::VISIBLE | nwg::RichLabelFlags::DISABLED)
            .parent(&self.status_window)
            .build(&mut self.status_shortcut_header)?;

        nwg::RichTextBox::builder()
            .position((MARGIN, SHORTCUT_TEXT_Y))
            .size((CONTENT_W, SHORTCUT_TEXT_H))
            .font(Some(&self.detail_font))
            .readonly(false)
            .flags(info_text_flags)
            .parent(&self.status_window)
            .build(&mut self.status_shortcut_text)?;

        self.status_shortcut_text.set_background_color(COLOR_WINDOW);

        let selection = self.shortcut_selection.borrow();
        let (edge_report, shortcut_report) = build_status_reports(&selection, None);
        self.status_edge_text.set_text(&edge_report);
        self.status_shortcut_text.set_text(&shortcut_report);
        set_rich_text_box_format_rect(&self.status_edge_text, 10, 8, 10, 8);
        set_rich_text_box_format_rect(&self.status_shortcut_text, 10, 8, 10, 8);
        self.status_edge_text.set_readonly(true);
        self.status_shortcut_text.set_readonly(true);

        nwg::Button::builder()
            .text("OK")
            .position((BUTTON_X, BUTTON_Y))
            .size((BUTTON_W, BUTTON_H))
            .parent(&self.status_window)
            .build(&mut self.button_status_ok)?;

        self.status_window.set_visible(false);

        Ok(())
    }

    fn build_tooltips(&mut self) -> Result<(), nwg::NwgError> {
        let standalone_text = "Example: --disable-extensions --force-dark-mode --mute-audio";
        let enable_text = "Example: msForceNoRoundedCornerAndMargin,msUndersideButton,ParallelDownloading";
        let disable_text = "Example: msShowSignInIndicator,msHubAppsSidebarRetirement,MediaRouter";

        nwg::Tooltip::builder()
            .register(&self.button_shortcuts, "Advanced")
            .register(&self.button_custom, "Custom")
            .register(&self.button_help, "Help")
            .register(&self.button_info, "Info")
            .register(&self.button_old, "--disable-features=msFeatureGroupNewLookAndFeelHoldout")
            .register(&self.button_new, "--enable-features=msForceNoRoundedCornerAndMargin")
            .register(&self.button_default, "Restore normal Edge shortcut settings")
            .register(&self.check_sign_in_indicator, "--disable-features=msShowSignInIndicator")
            .register(&self.check_restore_sidebar, "--disable-features=msHubAppsSidebarRetirement")
            .register(&self.check_disable_extensions, "--disable-extensions")
            .register(&self.check_external_links, "Apply flags when Edge is opened by other apps")
            .register(&self.check_shortcut_stable, "Microsoft Edge.lnk")
            .register(&self.check_shortcut_beta, "Microsoft Edge Beta.lnk")
            .register(&self.check_shortcut_dev, "Microsoft Edge Dev.lnk")
            .register(&self.check_shortcut_canary, "Microsoft Edge Canary.lnk")
            .register(&self.label_standalone_flags, standalone_text)
            .register(&self.text_standalone_flags, standalone_text)
            .register(&self.label_enable_features, enable_text)
            .register(&self.text_enable_features, enable_text)
            .register(&self.label_disable_features, disable_text)
            .register(&self.text_disable_features, disable_text)
            .build(&mut self.tooltip)?;

        Ok(())
    }

    fn load_settings(&self) {
        let settings = shortcut::load_app_settings();

        self.check_sign_in_indicator.set_check_state(check_box_state(settings.hide_sign_in_indicator));
        self.check_restore_sidebar.set_check_state(check_box_state(settings.restore_sidebar));
        self.check_disable_extensions.set_check_state(check_box_state(settings.disable_extensions));
        self.check_external_links.set_check_state(check_box_state(settings.apply_external_links));
        self.set_shortcuts_checked(&settings.shortcut_selection);
        *self.shortcut_selection.borrow_mut() = settings.shortcut_selection;
        *self.apply_external_links.borrow_mut() = settings.apply_external_links;
    }

    fn save_settings(&self) {
        let settings = shortcut::AppSettings {
            hide_sign_in_indicator: self.hide_sign_in_indicator_checked(),
            restore_sidebar: self.restore_sidebar_checked(),
            disable_extensions: self.disable_extensions_checked(),
            apply_external_links: self.apply_external_links_checked(),
            shortcut_selection: self.shortcut_selection.borrow().clone()
        };

        shortcut::save_app_settings(&settings);
    }

    fn process_event(&self, event: nwg::Event, handle: nwg::ControlHandle) {
        match event {
            nwg::Event::OnWindowClose => {
                if handle == self.window.handle {
                    nwg::stop_thread_dispatch();
                } else if handle == self.shortcuts_window.handle {
                    self.close_shortcuts();
                } else if handle == self.custom_window.handle {
                    self.close_custom_flags();
                } else if handle == self.help_window.handle {
                    self.close_help();
                } else if handle == self.status_window.handle {
                    self.close_status_info();
                }
            }

            nwg::Event::OnButtonClick => {
                if handle == self.button_exit.handle {
                    nwg::stop_thread_dispatch();
                } else if handle == self.button_shortcuts.handle {
                    self.show_shortcuts();
                } else if handle == self.button_help.handle {
                    self.show_help();
                } else if handle == self.button_info.handle {
                    self.show_status_info();
                } else if handle == self.button_custom.handle {
                    self.show_custom_flags();
                } else if handle == self.button_old.handle {
                    self.apply_old_workaround();
                } else if handle == self.button_new.handle {
                    self.apply_new_workaround();
                } else if handle == self.button_default.handle {
                    self.apply_options("", "Restore default");
                } else if handle == self.button_shortcuts_ok.handle {
                    self.save_shortcuts();
                } else if handle == self.button_shortcuts_cancel.handle {
                    self.close_shortcuts();
                } else if handle == self.button_apply_custom.handle {
                    self.apply_custom_flags();
                } else if handle == self.button_close_custom.handle {
                    self.close_custom_flags();
                } else if handle == self.button_help_ok.handle {
                    self.close_help();
                } else if handle == self.button_status_ok.handle {
                    self.close_status_info();
                }
            }

            _ => {}
        }
    }

    fn hide_sign_in_indicator_checked(&self) -> bool {
        self.check_sign_in_indicator.check_state() == nwg::CheckBoxState::Checked
    }

    fn restore_sidebar_checked(&self) -> bool {
        self.check_restore_sidebar.check_state() == nwg::CheckBoxState::Checked
    }

    fn disable_extensions_checked(&self) -> bool {
        self.check_disable_extensions.check_state() == nwg::CheckBoxState::Checked
    }

    fn apply_external_links_checked(&self) -> bool {
        *self.apply_external_links.borrow()
    }

    fn shortcuts_selected(&self) -> shortcut::ShortcutTargetSelection {
        shortcut::ShortcutTargetSelection {
            stable: self.check_shortcut_stable.check_state() == nwg::CheckBoxState::Checked,
            beta: self.check_shortcut_beta.check_state() == nwg::CheckBoxState::Checked,
            dev: self.check_shortcut_dev.check_state() == nwg::CheckBoxState::Checked,
            canary: self.check_shortcut_canary.check_state() == nwg::CheckBoxState::Checked
        }
    }

    fn set_shortcuts_checked(&self, selection: &shortcut::ShortcutTargetSelection) {
        self.check_shortcut_stable.set_check_state(check_box_state(selection.stable));
        self.check_shortcut_beta.set_check_state(check_box_state(selection.beta));
        self.check_shortcut_dev.set_check_state(check_box_state(selection.dev));
        self.check_shortcut_canary.set_check_state(check_box_state(selection.canary));
    }

    fn get_standalone_options(&self) -> String {
        if self.disable_extensions_checked() {
            DISABLE_EXTENSIONS_OPTION.to_string()
        } else {
            String::new()
        }
    }

    fn get_options_with_standalone(&self, feature_options: &str) -> String {
        let standalone_options = self.get_standalone_options();

        if standalone_options.is_empty() {
            feature_options.to_string()
        } else {
            format!("{} {}", standalone_options, feature_options)
        }
    }

    fn get_disable_features(&self, base_features: &[&str]) -> String {
        let mut features = Vec::new();

        if self.hide_sign_in_indicator_checked() {
            features.push(SIGN_IN_FEATURE);
        }

        if self.restore_sidebar_checked() {
            features.push(SIDEBAR_FEATURE);
        }

        features.extend_from_slice(base_features);
        features.join(",")
    }

    fn get_old_options(&self) -> String {
        let disable_features = self.get_disable_features(&[OLD_FEATURE]);
        let feature_options = format!("--disable-features=\"{}\"", disable_features);

        self.get_options_with_standalone(&feature_options)
    }

    fn get_new_options(&self) -> String {
        let disable_features = self.get_disable_features(&[]);
        let feature_options = if disable_features.is_empty() {
            format!("--enable-features=\"{}\"", NEW_FEATURE)
        } else {
            format!("--enable-features=\"{}\" --disable-features=\"{}\"", NEW_FEATURE, disable_features)
        };

        self.get_options_with_standalone(&feature_options)
    }

    fn apply_old_workaround(&self) {
        let options = self.get_old_options();
        self.apply_options(&options, "Old workaround");
    }

    fn apply_new_workaround(&self) {
        let options = self.get_new_options();
        self.apply_options(&options, "New workaround");
    }

    fn apply_custom_flags(&self) {
        let standalone_text = self.text_standalone_flags.text();
        let enable_text = self.text_enable_features.text();
        let disable_text = self.text_disable_features.text();

        let options = shortcut::get_custom_options_from_text(&standalone_text, &enable_text, &disable_text);

        if options.trim().is_empty() {
            self.status_box.set_text("Enter custom flags");
            self.custom_window.set_focus();
            return;
        }

        self.apply_options(&options, "Custom");
        self.custom_window.set_focus();
    }

    fn apply_options(&self, options: &str, mode_name: &str) {
        let selection = self.shortcut_selection.borrow().clone();
        let apply_external_links = self.apply_external_links_checked();

        self.status_box.set_text("Applying...");

        let result = shortcut::apply_options(options, &selection, apply_external_links);
        let external_failed = result
            .external_links
            .as_ref()
            .map(|external| matches!(&external.state, shortcut::ExternalLinkApplyState::Failed))
            .unwrap_or(false);

        let status_message = if result.failed > 0 || external_failed {
            "Completed with errors".to_string()
        } else {
            format!("Applied: {}", mode_name)
        };

        self.status_box.set_text(&status_message);
        *self.last_apply_result.borrow_mut() = Some(result);
        self.save_settings();
    }

    fn show_shortcuts(&self) {
        let selection = self.shortcut_selection.borrow();
        self.set_shortcuts_checked(&selection);
        self.check_external_links.set_check_state(check_box_state(*self.apply_external_links.borrow()));

        set_window_enabled(&self.window, false);
        center_window_on_cursor_monitor(&self.shortcuts_window);
        self.shortcuts_window.set_visible(true);
        self.shortcuts_window.set_focus();
        self.check_shortcut_stable.set_focus();
    }

    fn save_shortcuts(&self) {
        let selection = self.shortcuts_selected();
        let apply_external_links = self.check_external_links.check_state() == nwg::CheckBoxState::Checked;

        *self.shortcut_selection.borrow_mut() = selection;
        *self.apply_external_links.borrow_mut() = apply_external_links;
        self.save_settings();
        self.close_shortcuts();
    }

    fn close_shortcuts(&self) {
        self.shortcuts_window.set_visible(false);
        set_window_enabled(&self.window, true);
        self.window.set_focus();
    }

    fn show_custom_flags(&self) {
        let selection = self.shortcut_selection.borrow();
        let current_arguments = shortcut::get_current_shortcut_arguments(&selection);
        let standalone_options = shortcut::get_standalone_options_from_arguments(&current_arguments);
        let enable_features = shortcut::get_feature_list_from_arguments(&current_arguments, "enable-features");
        let disable_features = shortcut::get_feature_list_from_arguments(&current_arguments, "disable-features");

        self.text_standalone_flags.set_text(&standalone_options);
        self.text_enable_features.set_text(&enable_features);
        self.text_disable_features.set_text(&disable_features);

        set_window_enabled(&self.window, false);
        center_window_on_cursor_monitor_with_offset(&self.custom_window, 0, CUSTOM_WINDOW_CENTER_Y_OFFSET);
        self.custom_window.set_visible(true);
        self.custom_window.set_focus();
        self.text_standalone_flags.set_focus();
    }

    fn close_custom_flags(&self) {
        self.custom_window.set_visible(false);
        set_window_enabled(&self.window, true);
        self.window.set_focus();
    }

    fn show_help(&self) {
        set_window_enabled(&self.window, false);
        center_window_on_cursor_monitor(&self.help_window);
        self.help_window.set_visible(true);
        self.help_window.set_focus();
        self.button_help_ok.set_focus();
    }

    fn close_help(&self) {
        self.help_window.set_visible(false);
        set_window_enabled(&self.window, true);
        self.window.set_focus();
    }

    fn show_status_info(&self) {
        let (edge_report, shortcut_report) = {
            let selection = self.shortcut_selection.borrow();
            let last_apply_result = self.last_apply_result.borrow();
            build_status_reports(&selection, last_apply_result.as_ref())
        };

        // Temporarily unlock the boxes so their text can be refreshed.
        self.status_edge_text.set_readonly(false);
        self.status_edge_text.set_text(&edge_report);
        self.status_edge_text.set_selection(0..0);
        self.status_edge_text.set_readonly(true);

        self.status_shortcut_text.set_readonly(false);
        self.status_shortcut_text.set_text(&shortcut_report);
        self.status_shortcut_text.set_selection(0..0);
        self.status_shortcut_text.set_readonly(true);

        set_window_enabled(&self.window, false);
        center_window_on_cursor_monitor(&self.status_window);
        self.status_window.set_visible(true);
        self.status_window.set_focus();
        self.button_status_ok.set_focus();
    }

    fn close_status_info(&self) {
        self.status_window.set_visible(false);
        set_window_enabled(&self.window, true);
        self.window.set_focus();
    }

    fn style_help_text(&self) {
        // Apply formatting after the help text is loaded into RichEdit.
        self.help_text.set_text(HELP_TEXT);

        self.apply_help_format(0, HELP_TEXT.len(), nwg::CharEffects::empty(), BLACK);

        for title in [
            "Old workaround",
            "New workaround",
            "Restore default",
            "Hide sign-in red dot",
            "Restore sidebar",
            "Disable extensions",
            "Advanced",
            "Custom",
            "Info"
        ] {
            self.apply_all_help_text_format(title, nwg::CharEffects::BOLD, BLACK);
        }

        self.apply_all_help_text_format("Reminder", nwg::CharEffects::BOLD, NOTE_RED);

        self.apply_all_help_text_format("edge://settings/system/manageSystem", nwg::CharEffects::UNDERLINE, LINK_BLUE);

        set_rich_text_box_format_rect(&self.help_text, 4, 4, 4, 2);
        self.help_text.set_selection(0..0);
        self.help_text.set_readonly(true);
    }

    fn apply_all_help_text_format(&self, needle: &str, effects: nwg::CharEffects, color: [u8; 3]) {
        let mut search_start = 0usize;

        while let Some(offset) = HELP_TEXT[search_start..].find(needle) {
            let start = search_start + offset;
            let end = start + needle.len();

            self.apply_help_format(start, end, effects, color);
            search_start = end;
        }
    }

    fn apply_help_format(&self, start: usize, end: usize, effects: nwg::CharEffects, color: [u8; 3]) {
        let selection_start = rich_edit_position(HELP_TEXT, start);
        let selection_end = rich_edit_position(HELP_TEXT, end);

        self.help_text.set_selection(selection_start..selection_end);

        let format = nwg::CharFormat {
            effects: Some(effects),
            height: None,
            y_offset: None,
            text_color: Some(color),
            font_face_name: Some("Segoe UI".to_string()),
            underline_type: None
        };

        self.help_text.set_char_format(&format);
    }

    fn harden_window_chrome(&self) {
        harden_window_chrome(&self.window);
        harden_window_chrome(&self.shortcuts_window);
        harden_window_chrome(&self.custom_window);
        harden_window_chrome(&self.help_window);
        harden_window_chrome(&self.status_window);
    }

    fn center_main_window(&self) {
        center_window_on_cursor_monitor(&self.window);
    }
}

fn check_box_state(checked: bool) -> nwg::CheckBoxState {
    if checked {
        nwg::CheckBoxState::Checked
    } else {
        nwg::CheckBoxState::Unchecked
    }
}

fn rich_edit_position(text: &str, byte_index: usize) -> u32 {
    // Convert Rust byte positions to RichEdit selection positions.
    let mut position = 0u32;
    let mut index = 0usize;
    let bytes = text.as_bytes();

    while index < byte_index {
        if index + 1 < byte_index && bytes[index] == b'\r' && bytes[index + 1] == b'\n' {
            // RichEdit treats CRLF as one character position.
            position += 1;
            index += 2;
            continue;
        }

        let Some(ch) = text[index..].chars().next() else {
            break;
        };

        position += ch.len_utf16() as u32;
        index += ch.len_utf8();
    }

    position
}

fn build_status_reports(selection: &shortcut::ShortcutTargetSelection, last_apply_result: Option<&shortcut::ApplyResult>) -> (String, String) {
    // Build each section separately so the Info window can size them independently.
    let mut edge_report = String::new();
    let candidates = shortcut::get_edge_executable_candidates();
    write_edge_candidate_section(&mut edge_report, &candidates);

    let mut shortcut_report = String::new();
    write_shortcut_section(&mut shortcut_report, selection, last_apply_result);

    (
        edge_report.replace('\n', "\r\n"),
        shortcut_report.replace('\n', "\r\n")
    )
}

fn write_shortcut_section(report: &mut String, selection: &shortcut::ShortcutTargetSelection, last_apply_result: Option<&shortcut::ApplyResult>) {
    if let Some(result) = last_apply_result {
        write_selected_shortcuts(report, &result.selected_shortcut_names);

        let updated = result
            .details
            .iter()
            .filter(|detail| matches!(&detail.state, shortcut::ShortcutApplyState::Updated))
            .collect::<Vec<_>>();

        let failed = result
            .details
            .iter()
            .filter(|detail| matches!(&detail.state, shortcut::ShortcutApplyState::Failed))
            .collect::<Vec<_>>();

        let missing = result
            .details
            .iter()
            .filter(|detail| matches!(&detail.state, shortcut::ShortcutApplyState::IgnoredMissing))
            .collect::<Vec<_>>();

        write_shortcut_group(report, "Updated", &updated);
        write_shortcut_group(report, "Failed", &failed);
        write_shortcut_group(report, "Missing", &missing);

        write_external_link_section(report);

        let _ = writeln!(report);
        let _ = writeln!(
            report, "Latest summary: {} found, {} updated, {} missing, {} failed",
            result.found_shortcuts, updated.len(), missing.len(), failed.len()
        );
    } else {
        let selected_shortcuts = shortcut::selected_shortcut_display_names(selection);
        write_selected_shortcuts(report, &selected_shortcuts);

        let mut found = Vec::new();
        let mut missing = Vec::new();

        for path in shortcut::get_shortcut_paths(selection) {
            if path.exists() {
                found.push(path);
            } else {
                missing.push(path);
            }
        }

        if !found.is_empty() {
            let _ = writeln!(report, "Found:");

            for path in found {
                write_path_entry(report, shortcut_location_label(&path), &path);
            }

            let _ = writeln!(report);
        }

        if !missing.is_empty() {
            let _ = writeln!(report, "Missing:");

            for path in missing {
                write_path_entry(report, shortcut_location_label(&path), &path);
            }

            let _ = writeln!(report);
        }

        write_external_link_section(report);

        let _ = writeln!(report);
        let _ = writeln!(report, "No operation has run during this session.");
    }
}

fn write_external_link_section(report: &mut String) {
    let _ = writeln!(report, "Registry:");
    let _ = writeln!(report, "  External links:");
    let _ = writeln!(report, "    [{}]", shortcut::external_link_command_registry_key());

    if let Some(command) = shortcut::get_current_user_external_link_command() {
        write_external_link_command(report, &command);
    } else {
        let _ = writeln!(report, "    None applied");
    }
}

fn write_external_link_command(report: &mut String, command: &str) {
    if let Some((program, arguments)) = command.split_once("\" --") {
        let _ = writeln!(report, "    {}\"", program);
        write_external_link_arguments(report, &format!("--{}", arguments));
    } else {
        let _ = writeln!(report, "    {}", command);
    }
}

fn write_external_link_arguments(report: &mut String, arguments: &str) {
    let mut first = true;

    for part in arguments.split(" --") {
        let part = part.trim();

        if part.is_empty() {
            continue;
        }

        if first {
            let _ = writeln!(report, "    {}", part);
            first = false;
        } else {
            let _ = writeln!(report, "    --{}", part);
        }
    }
}

fn write_selected_shortcuts(report: &mut String, names: &[&str]) {
    if names.is_empty() {
        let _ = writeln!(report, "Selected: None");
    } else {
        let _ = writeln!(report, "Selected: {}", names.join(", "));
    }

    let _ = writeln!(report);
}

fn write_edge_candidate_section(report: &mut String, candidates: &[shortcut::EdgeExecutableCandidate]) {
    let selected = candidates.iter().find(|candidate| candidate.selected);

    let _ = writeln!(report, "Selected:");

    if let Some(candidate) = selected {
        write_candidate_entry(report, candidate, "");
    } else {
        let _ = writeln!(report, "  Not found in common install locations");
    }

    let _ = writeln!(report);
    let _ = writeln!(report, "Candidates:");

    for candidate in candidates {
        let status = if candidate.exists {
            "Exists"
        } else if candidate.path.is_some() {
            "Missing"
        } else {
            "Environment missing"
        };

        write_candidate_entry(report, candidate, status);
    }

    let _ = writeln!(report);
    let _ = writeln!(report, "For reference and diagnostic purposes only.");
}

fn write_candidate_entry(report: &mut String, candidate: &shortcut::EdgeExecutableCandidate, status: &str) {
    let path_text = candidate
        .path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "(environment variable not set)".to_string());

    if status.is_empty() {
        let _ = writeln!(report, "  {}", candidate.source);
    } else {
        let _ = writeln!(report, "  {} - {}", status, candidate.source);
    }
    let _ = writeln!(report, "    {}", path_text);
}

fn write_shortcut_group(report: &mut String, title: &str, details: &[&shortcut::ShortcutApplyDetail]) {
    if details.is_empty() {
        return;
    }

    let _ = writeln!(report, "{}:", title);

    for detail in details {
        write_path_entry(report, shortcut_location_label(&detail.path), &detail.path);
    }

    let _ = writeln!(report);
}

fn write_path_entry(report: &mut String, label: &str, path: &Path) {
    let _ = writeln!(report, "  {}", label);
    let _ = writeln!(report, "    {}", path.display());
}

fn shortcut_location_label(path: &Path) -> &'static str {
    let value = path.to_string_lossy().to_ascii_lowercase();

    if value.contains(r"\programdata\microsoft\windows\start menu\") {
        "System Start Menu"
    } else if value.contains(r"\users\public\desktop\") || value.contains(r"\public\desktop\") {
        "Public Desktop"
    } else if value.contains(r"\user pinned\taskbar\") {
        "Pinned Taskbar"
    } else if value.contains(r"\user pinned\startmenu\") {
        "Pinned Start Menu"
    } else if value.contains(r"\implicitappshortcuts\") {
        "Implicit pinned shortcut"
    } else if value.contains(r"\internet explorer\quick launch\") {
        "Quick Launch"
    } else if value.contains(r"\appdata\roaming\microsoft\windows\start menu\") {
        "Current user Start Menu"
    } else if value.contains(r"\desktop\") {
        "Current user Desktop"
    } else {
        "Shortcut"
    }
}

fn show_plain_dialog(message: &str) {
    let _ = nwg::simple_message(APP_TITLE, message);
}

fn get_hwnd(window: &nwg::Window) -> Option<HWND> {
    window.handle.hwnd().map(|hwnd| HWND(hwnd as *mut core::ffi::c_void))
}

fn get_control_hwnd(handle: &nwg::ControlHandle) -> Option<HWND> {
    handle.hwnd().map(|hwnd| HWND(hwnd as *mut core::ffi::c_void))
}

fn set_rich_text_box_format_rect(control: &nwg::RichTextBox, left: i32, top: i32, right: i32, bottom: i32) {
    // Add inner padding to RichEdit controls without changing their outer size.
    let Some(hwnd) = get_control_hwnd(&control.handle) else {
        return;
    };

    let (width, height) = control.size();

    if width == 0 || height == 0 {
        return;
    }

    let rect = RECT {
        left,
        top,
        right: width as i32 - right,
        bottom: height as i32 - bottom
    };

    // SAFETY: hwnd belongs to this RichTextBox, and rect lives for the
    // synchronous EM_SETRECT message. RichEdit does not retain this pointer.
    unsafe {
        let _ = SendMessageW(
            hwnd,
            EM_SETRECT_MESSAGE,
            Some(WPARAM(0)),
            Some(LPARAM((&rect as *const RECT) as isize))
        );
    }
}

fn set_window_enabled(window: &nwg::Window, enabled: bool) {
    let Some(hwnd) = get_hwnd(window) else {
        return;
    };

    // SAFETY: hwnd was obtained from a live NWG window handle. EnableWindow
    // changes window state synchronously and does not retain pointers.
    unsafe {
        let _ = EnableWindow(hwnd, enabled);
    }
}

fn harden_window_chrome(window: &nwg::Window) {
    // Keep all app windows as fixed, icon-less dialog-style windows.
    let Some(hwnd) = get_hwnd(window) else {
        return;
    };

    // SAFETY: hwnd was obtained from a live NWG window. These Win32 calls
    // synchronously update window icon, style, menu, and frame state.
    unsafe {
        let _ = SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_SMALL as usize)),
            Some(LPARAM(0))
        );

        let _ = SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_BIG as usize)),
            Some(LPARAM(0))
        );

        let _ = SetClassLongPtrW(hwnd, GCLP_HICONSM, 0);
        let _ = SetClassLongPtrW(hwnd, GCLP_HICON, 0);

        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let _ = SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            ex_style | WS_EX_DLGMODALFRAME.0 as isize
        );

        let system_menu = GetSystemMenu(hwnd, false);

        if !system_menu.0.is_null() {
            let _ = DeleteMenu(system_menu, SC_RESTORE, MF_BYCOMMAND);
            let _ = DeleteMenu(system_menu, SC_SIZE, MF_BYCOMMAND);
            let _ = DeleteMenu(system_menu, SC_MINIMIZE, MF_BYCOMMAND);
            let _ = DeleteMenu(system_menu, SC_MAXIMIZE, MF_BYCOMMAND);
            let _ = DrawMenuBar(hwnd);
        }

        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOZORDER
                | SWP_NOACTIVATE
                | SWP_FRAMECHANGED
        );
    }
}

fn center_window_on_cursor_monitor(window: &nwg::Window) {
    center_window_on_cursor_monitor_with_offset(window, 0, 0);
}

fn center_window_on_cursor_monitor_with_offset(window: &nwg::Window, offset_x: i32, offset_y: i32) {
    // Center popups on the monitor where the user is currently working.
    let Some(hwnd) = get_hwnd(window) else {
        return;
    };

    let (window_width, window_height) = window.size();

    // SAFETY: Uses initialized stack structs, validates Win32 return values,
    // and passes no pointers that outlive the synchronous API calls.
    unsafe {
        let mut cursor = POINT { x: 0, y: 0 };

        if GetCursorPos(&mut cursor).is_err() {
            return;
        }

        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);

        if monitor.0.is_null() {
            return;
        }

        let mut monitor_info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: Default::default(),
            rcWork: Default::default(),
            dwFlags: 0
        };

        if !GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
            return;
        }

        let work = monitor_info.rcWork;
        let work_width = work.right - work.left;
        let work_height = work.bottom - work.top;

        let centered_x = work.left + (work_width - window_width as i32) / 2;
        let centered_y = work.top + (work_height - window_height as i32) / 2;
        let x = (centered_x + offset_x).clamp(work.left, work.right - window_width as i32);
        let y = (centered_y + offset_y).clamp(work.top, work.bottom - window_height as i32);

        let _ = SetWindowPos(
            hwnd,
            None,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE
        );
    }
}

fn set_global_ui_font() -> Result<(), nwg::NwgError> {
    let mut font = nwg::Font::default();

    nwg::Font::builder()
        .family("Segoe UI")
        .size_absolute(12)
        .build(&mut font)?;

    let _old_font = nwg::Font::set_global_default(Some(font));

    Ok(())
}

fn bind_events(app: &Rc<App>) -> (
    nwg::EventHandler,
    nwg::EventHandler,
    nwg::EventHandler,
    nwg::EventHandler,
    nwg::EventHandler
) {
    // Each top-level window has its own handler so child button clicks are captured.
    let main_handle = app.window.handle.clone();
    let main_app = Rc::clone(app);

    let main_events = nwg::full_bind_event_handler(
        &main_handle,
        move |event, _event_data, handle| {
            main_app.process_event(event, handle);
        }
    );

    let shortcuts_handle = app.shortcuts_window.handle.clone();
    let shortcuts_app = Rc::clone(app);

    let shortcuts_events = nwg::full_bind_event_handler(
        &shortcuts_handle,
        move |event, _event_data, handle| {
            shortcuts_app.process_event(event, handle);
        }
    );

    let custom_handle = app.custom_window.handle.clone();
    let custom_app = Rc::clone(app);

    let custom_events = nwg::full_bind_event_handler(
        &custom_handle,
        move |event, _event_data, handle| {
            custom_app.process_event(event, handle);
        }
    );

    let help_handle = app.help_window.handle.clone();
    let help_app = Rc::clone(app);

    let help_events = nwg::full_bind_event_handler(
        &help_handle,
        move |event, _event_data, handle| {
            help_app.process_event(event, handle);
        }
    );

    let status_handle = app.status_window.handle.clone();
    let status_app = Rc::clone(app);

    let status_events = nwg::full_bind_event_handler(
        &status_handle,
        move |event, _event_data, handle| {
            status_app.process_event(event, handle);
        }
    );

    (main_events, shortcuts_events, custom_events, help_events, status_events)
}

fn main() {
    if let Err(error) = nwg::init() {
        eprintln!("failed to initialize native-windows-gui: {}", error);
        return;
    }

    if let Err(error) = set_global_ui_font() {
        show_plain_dialog(&format!("Failed to initialize Segoe UI font.\r\n\r\n{}", error));
        return;
    }

    let _com = match shortcut::ComApartment::init() {
        Ok(com) => com,
        Err(error) => {
            show_plain_dialog(&format!("Failed to initialize COM.\r\n\r\n{}", error));
            return;
        }
    };

    let app = match App::build() {
        Ok(app) => Rc::new(app),
        Err(error) => {
            show_plain_dialog(&format!("Failed to build the user interface.\r\n\r\n{}", error));
            return;
        }
    };

    let _handlers = bind_events(&app);

    nwg::dispatch_thread_events();
}
