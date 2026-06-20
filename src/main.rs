#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod shortcut;

use std::cell::RefCell;
use std::fmt::Write;
use std::path::Path;
use std::rc::Rc;

use native_windows_gui as nwg;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    DeleteMenu, DrawMenuBar, GetCursorPos, GetSystemMenu, GetWindowLongPtrW, SendMessageW,
    SetClassLongPtrW, SetWindowLongPtrW, SetWindowPos, GCLP_HICON, GCLP_HICONSM,
    GWL_EXSTYLE, ICON_BIG, ICON_SMALL, MF_BYCOMMAND, SC_MAXIMIZE, SC_MINIMIZE,
    SC_RESTORE, SC_SIZE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, WM_SETICON, WS_EX_DLGMODALFRAME,
};

const APP_TITLE: &str = "Edge Shortcut Tool";

const OLD_FEATURE: &str = "msFeatureGroupNewLookAndFeelHoldout";
const NEW_FEATURE: &str = "msForceNoRoundedCornerAndMargin";
const SIGN_IN_FEATURE: &str = "msShowSignInIndicator";

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
    "This is the older/broader method for removing Edge rounded corner look.\r\n",
    "\r\n",
    "New workaround\r\n",
    "Uses msForceNoRoundedCornerAndMargin.\r\n",
    "This is the newer/more direct method for removing rounded corners and margins.\r\n",
    "\r\n",
    "Restore default\r\n",
    "Removes existing Edge enable/disable feature flags from supported shortcuts.\r\n",
    "Other shortcut arguments, such as profile arguments, are preserved.\r\n",
    "\r\n",
    "Hide sign-in red dot\r\n",
    "When checked, this also disables msShowSignInIndicator.\r\n",
    "This hides the red sign-in reminder dot on Edge profile icon.\r\n",
    "\r\n",
    "Custom\r\n",
    "Enter feature names separated by commas in the enable/disable fields.\r\n",
    "Applying custom flags replaces existing enable/disable feature flags.\r\n",
    "\r\n",
    "Info\r\n",
    "Displays executable candidates and supported shortcut locations.\r\n",
    "Also shows which shortcuts were updated, missing, or failed.\r\n",
    "\r\n",
    "Reminder\r\n",
    "Disable Startup Boost in Edge first. Copy and paste this into Edge address bar: ",
    "edge://settings/system/manageSystem\r\n"
);

#[derive(Default)]
struct App {
    window: nwg::Window,

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

    status_box: nwg::RichTextBox,
    button_info: nwg::Button,
    button_exit: nwg::Button,

    custom_window: nwg::Window,
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
    tooltip: nwg::Tooltip,
    last_apply_result: RefCell<Option<shortcut::ApplyResult>>,
}

impl App {
    fn build() -> Result<Self, nwg::NwgError> {
        let mut app = App::default();

        app.build_fonts()?;
        app.build_main_window()?;
        app.build_custom_window()?;
        app.build_help_window()?;
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

        Ok(())
    }

    fn build_main_window(&mut self) -> Result<(), nwg::NwgError> {
        nwg::Window::builder()
            .flags(nwg::WindowFlags::WINDOW)
            .size((312, 263))
            .position((400, 300))
            .title(APP_TITLE)
            .build(&mut self.window)?;

        nwg::Button::builder()
            .text("...")
            .position((239, 12))
            .size((27, 25))
            .font(Some(&self.info_font))
            .parent(&self.window)
            .build(&mut self.button_custom)?;

        nwg::Button::builder()
            .text("?")
            .position((271, 12))
            .size((27, 25))
            .font(Some(&self.info_font))
            .parent(&self.window)
            .build(&mut self.button_help)?;

        // Draw the preset border manually so the group line stays soft and consistent.
        nwg::Frame::builder()
            .flags(nwg::FrameFlags::VISIBLE)
            .position((14, 42))
            .size((284, 140))
            .parent(&self.window)
            .build(&mut self.preset_panel)?;

        nwg::RichLabel::builder()
            .text("")
            .position((14, 42))
            .size((9, 1))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.window)
            .build(&mut self.preset_line_top_left)?;

        nwg::RichLabel::builder()
            .text("")
            .position((128, 42))
            .size((170, 1))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.window)
            .build(&mut self.preset_line_top_right)?;

        nwg::RichLabel::builder()
            .text("")
            .position((14, 42))
            .size((1, 140))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.window)
            .build(&mut self.preset_line_left)?;

        nwg::RichLabel::builder()
            .text("")
            .position((297, 42))
            .size((1, 140))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.window)
            .build(&mut self.preset_line_right)?;

        nwg::RichLabel::builder()
            .text("")
            .position((14, 181))
            .size((284, 1))
            .background_color(Some(COLOR_GROUP_LINE))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.window)
            .build(&mut self.preset_line_bottom)?;

        nwg::RichLabel::builder()
            .text("Fix rounded corners")
            .position((24, 36))
            .size((104, 20))
            .font(Some(&self.ui_font))
            .background_color(Some(COLOR_CONTROL))
            .flags(nwg::RichLabelFlags::VISIBLE)
            .parent(&self.window)
            .build(&mut self.preset_title)?;

        nwg::Button::builder()
            .text("Old workaround")
            .position((16, 19))
            .size((252, 30))
            .parent(&self.preset_panel)
            .build(&mut self.button_old)?;

        nwg::Button::builder()
            .text("New workaround")
            .position((16, 55))
            .size((252, 30))
            .parent(&self.preset_panel)
            .build(&mut self.button_new)?;

        nwg::Button::builder()
            .text("Restore default")
            .position((16, 91))
            .size((252, 30))
            .parent(&self.preset_panel)
            .build(&mut self.button_default)?;

        nwg::CheckBox::builder()
            .text("Hide sign-in red dot")
            .position((16, 190))
            .size((250, 22))
            .parent(&self.window)
            .build(&mut self.check_sign_in_indicator)?;

        self.check_sign_in_indicator
            .set_check_state(nwg::CheckBoxState::Checked);

        // RichTextBox gives the status field the same sunken Win32 look as the dialogs.
        nwg::RichTextBox::builder()
            .position((14, 225))
            .size((174, 24))
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
            .position((193, 224))
            .size((27, 25))
            .font(Some(&self.info_font))
            .parent(&self.window)
            .build(&mut self.button_info)?;

        nwg::Button::builder()
            .text("Exit")
            .position((225, 224))
            .size((73, 26))
            .parent(&self.window)
            .build(&mut self.button_exit)?;

        Ok(())
    }

    fn build_custom_window(&mut self) -> Result<(), nwg::NwgError> {
        const WINDOW_W: i32 = 470;
        const WINDOW_H: i32 = 160;

        const MARGIN_X: i32 = 16;
        const LABEL_W: i32 = WINDOW_W - (MARGIN_X * 2);
        const TEXT_W: i32 = LABEL_W;
        const TEXT_H: i32 = 24;

        const BUTTON_W: i32 = 75;
        const BUTTON_H: i32 = 26;
        const BUTTON_GAP: i32 = 8;
        const BOTTOM_MARGIN: i32 = 11;
        const BUTTON_Y: i32 = WINDOW_H - BOTTOM_MARGIN - BUTTON_H;
        const BUTTON_CLOSE_X: i32 = WINDOW_W - MARGIN_X - BUTTON_W;
        const BUTTON_APPLY_X: i32 = BUTTON_CLOSE_X - BUTTON_GAP - BUTTON_W;

        nwg::Window::builder()
            .flags(nwg::WindowFlags::WINDOW)
            .size((WINDOW_W, WINDOW_H))
            .position((430, 340))
            .title("Custom")
            .build(&mut self.custom_window)?;

        nwg::Label::builder()
            .text("Enable features:")
            .position((MARGIN_X, 18))
            .size((LABEL_W, 18))
            .parent(&self.custom_window)
            .build(&mut self.label_enable_features)?;

        nwg::TextInput::builder()
            .position((MARGIN_X, 37))
            .size((TEXT_W, TEXT_H))
            .parent(&self.custom_window)
            .build(&mut self.text_enable_features)?;

        nwg::Label::builder()
            .text("Disable features:")
            .position((MARGIN_X, 67))
            .size((LABEL_W, 18))
            .parent(&self.custom_window)
            .build(&mut self.label_disable_features)?;

        nwg::TextInput::builder()
            .position((MARGIN_X, 86))
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
        const WINDOW_W: i32 = 480;
        const WINDOW_H: i32 = 485;

        const TEXT_X: i32 = 14;
        const TEXT_Y: i32 = 14;
        const TEXT_W: i32 = WINDOW_W - (TEXT_X * 2);
        const TEXT_H: i32 = 426;

        const BUTTON_W: i32 = 75;
        const BUTTON_H: i32 = 26;
        const BUTTON_GAP_Y: i32 = 8;
        const BUTTON_X: i32 = TEXT_X + TEXT_W - BUTTON_W;
        const BUTTON_Y: i32 = TEXT_Y + TEXT_H + BUTTON_GAP_Y;

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
        const WINDOW_W: i32 = 750;

        const MARGIN: i32 = 16;
        const GAP: i32 = 8;
        const SECTION_GAP: i32 = 16;
        const BOTTOM_MARGIN: i32 = 11;
        const HEADER_H: i32 = 20;
        const HEADER_LINE_HEIGHT: i32 = 18;
        const HEADER_TEXT_GAP: i32 = 4;
        const CONTENT_W: i32 = WINDOW_W - (MARGIN * 2);

        const EDGE_HEADER_Y: i32 = MARGIN;
        const EDGE_TEXT_Y: i32 = EDGE_HEADER_Y + HEADER_H + HEADER_TEXT_GAP;
        const EDGE_TEXT_H: i32 = 236;

        const SHORTCUT_HEADER_Y: i32 = EDGE_TEXT_Y + EDGE_TEXT_H + SECTION_GAP;
        const SHORTCUT_TEXT_Y: i32 = SHORTCUT_HEADER_Y + HEADER_H + HEADER_TEXT_GAP;
        const SHORTCUT_TEXT_H: i32 = 328;

        const BUTTON_W: i32 = 75;
        const BUTTON_H: i32 = 26;
        const BUTTON_X: i32 = MARGIN + CONTENT_W - BUTTON_W;
        const BUTTON_Y: i32 = SHORTCUT_TEXT_Y + SHORTCUT_TEXT_H + GAP;
        const WINDOW_H: i32 = BUTTON_Y + BUTTON_H + BOTTOM_MARGIN;

        // Keep diagnostic sections scrollable when future output becomes longer.
        let info_text_flags = nwg::RichTextBoxFlags::VISIBLE
            | nwg::RichTextBoxFlags::SAVE_SELECTION
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

        let (edge_report, shortcut_report) = build_status_reports(None);
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
        let custom_text = "Enter comma-separated names. Example: FeatureA,FeatureB,FeatureC";

        nwg::Tooltip::builder()
            .register(&self.button_custom, "Custom")
            .register(&self.button_help, "Help")
            .register(&self.button_info, "Info")
            .register(
                &self.button_old,
                "--disable-features=msFeatureGroupNewLookAndFeelHoldout",
            )
            .register(
                &self.button_new,
                "--enable-features=msForceNoRoundedCornerAndMargin",
            )
            .register(&self.button_default, "Restore normal Edge shortcut settings")
            .register(
                &self.check_sign_in_indicator,
                "--disable-features=msShowSignInIndicator",
            )
            .register(&self.label_enable_features, custom_text)
            .register(&self.text_enable_features, custom_text)
            .register(&self.label_disable_features, custom_text)
            .register(&self.text_disable_features, custom_text)
            .build(&mut self.tooltip)?;

        Ok(())
    }

    fn process_event(&self, event: nwg::Event, handle: nwg::ControlHandle) {
        match event {
            nwg::Event::OnWindowClose => {
                if handle == self.window.handle {
                    nwg::stop_thread_dispatch();
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

    fn get_old_options(&self) -> String {
        if self.hide_sign_in_indicator_checked() {
            format!("--disable-features=\"{},{}\"", SIGN_IN_FEATURE, OLD_FEATURE)
        } else {
            format!("--disable-features=\"{}\"", OLD_FEATURE)
        }
    }

    fn get_new_options(&self) -> String {
        if self.hide_sign_in_indicator_checked() {
            format!(
                "--enable-features=\"{}\" --disable-features=\"{}\"",
                NEW_FEATURE, SIGN_IN_FEATURE
            )
        } else {
            format!("--enable-features=\"{}\"", NEW_FEATURE)
        }
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
        let enable_text = self.text_enable_features.text();
        let disable_text = self.text_disable_features.text();

        let options = shortcut::get_custom_options_from_text(&enable_text, &disable_text);

        if options.trim().is_empty() {
            self.status_box.set_text("Enter custom flags");
            self.custom_window.set_focus();
            return;
        }

        self.apply_options(&options, "Custom");
        self.custom_window.set_focus();
    }

    fn apply_options(&self, options: &str, mode_name: &str) {
        self.status_box.set_text("Applying...");

        let result = shortcut::apply_options(options);

        let status_message = if result.found_shortcuts == 0 {
            "No shortcuts found".to_string()
        } else if result.failed > 0 {
            "Completed with errors".to_string()
        } else {
            format!("Applied: {}", mode_name)
        };

        self.status_box.set_text(&status_message);
        *self.last_apply_result.borrow_mut() = Some(result);
    }

    fn show_custom_flags(&self) {
        let current_arguments = shortcut::get_current_shortcut_arguments();

        let enable_features =
            shortcut::get_feature_list_from_arguments(&current_arguments, "enable-features");

        let disable_features =
            shortcut::get_feature_list_from_arguments(&current_arguments, "disable-features");

        self.text_enable_features.set_text(&enable_features);
        self.text_disable_features.set_text(&disable_features);

        set_window_enabled(&self.window, false);
        center_window_on_cursor_monitor(&self.custom_window);
        self.custom_window.set_visible(true);
        self.custom_window.set_focus();
        self.text_enable_features.set_focus();
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
            let last_apply_result = self.last_apply_result.borrow();
            build_status_reports(last_apply_result.as_ref())
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
            "Custom",
            "Info",
        ] {
            self.apply_all_help_text_format(title, nwg::CharEffects::BOLD, BLACK);
        }

        self.apply_all_help_text_format("Reminder", nwg::CharEffects::BOLD, NOTE_RED);

        self.apply_all_help_text_format(
            "edge://settings/system/manageSystem",
            nwg::CharEffects::UNDERLINE,
            LINK_BLUE,
        );

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

    fn apply_help_format(
        &self,
        start: usize,
        end: usize,
        effects: nwg::CharEffects,
        color: [u8; 3],
    ) {
        let selection_start = rich_edit_position(HELP_TEXT, start);
        let selection_end = rich_edit_position(HELP_TEXT, end);

        self.help_text.set_selection(selection_start..selection_end);

        let format = nwg::CharFormat {
            effects: Some(effects),
            height: None,
            y_offset: None,
            text_color: Some(color),
            font_face_name: Some("Segoe UI".to_string()),
            underline_type: None,
        };

        self.help_text.set_char_format(&format);
    }

    fn harden_window_chrome(&self) {
        harden_window_chrome(&self.window);
        harden_window_chrome(&self.custom_window);
        harden_window_chrome(&self.help_window);
        harden_window_chrome(&self.status_window);
    }

    fn center_main_window(&self) {
        center_window_on_cursor_monitor(&self.window);
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

fn build_status_reports(last_apply_result: Option<&shortcut::ApplyResult>) -> (String, String) {
    // Build each section separately so the Info window can size them independently.
    let mut edge_report = String::new();
    let candidates = shortcut::get_edge_executable_candidates();
    write_edge_candidate_section(&mut edge_report, &candidates);

    let mut shortcut_report = String::new();
    write_shortcut_section(&mut shortcut_report, last_apply_result);

    (
        edge_report.replace('\n', "\r\n"),
        shortcut_report.replace('\n', "\r\n"),
    )
}

fn write_shortcut_section(
    report: &mut String,
    last_apply_result: Option<&shortcut::ApplyResult>,
) {
    if let Some(result) = last_apply_result {
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

        let _ = writeln!(
            report, "Latest summary: {} found, {} updated, {} missing, {} failed",
            result.found_shortcuts, updated.len(), missing.len(), failed.len()
        );
    } else {
        let mut found = Vec::new();
        let mut missing = Vec::new();

        for path in shortcut::get_shortcut_paths() {
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

        let _ = writeln!(report, "No operation has run during this session.");
    }
}

fn write_edge_candidate_section(
    report: &mut String,
    candidates: &[shortcut::EdgeExecutableCandidate],
) {
    let selected = candidates.iter().find(|candidate| candidate.selected);

    let _ = writeln!(report, "Selected:");

    if let Some(candidate) = selected {
        write_candidate_entry(report, candidate, "Selected");
    } else {
        let _ = writeln!(report, "  Not found in common install locations");
    }

    let _ = writeln!(report);
    let _ = writeln!(report, "Candidates:");

    for candidate in candidates {
        let status = if candidate.selected {
            "Selected"
        } else if candidate.exists {
            "Found"
        } else if candidate.path.is_some() {
            "Missing"
        } else {
            "Environment missing"
        };

        write_candidate_entry(report, candidate, status);
    }

    let _ = writeln!(report);
    let _ = writeln!(
        report,
        "For reference and diagnostic purposes only."
    );
}

fn write_candidate_entry(
    report: &mut String,
    candidate: &shortcut::EdgeExecutableCandidate,
    status: &str,
) {
    let path_text = candidate
        .path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "(environment variable not set)".to_string());

    let _ = writeln!(report, "  {} - {}", status, candidate.source);
    let _ = writeln!(report, "    {}", path_text);
}

fn write_shortcut_group(
    report: &mut String,
    title: &str,
    details: &[&shortcut::ShortcutApplyDetail],
) {
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
    } else if value.contains(r"\internet explorer\quick launch\microsoft edge.lnk") {
        "Quick Launch"
    } else if value.contains(r"\appdata\roaming\microsoft\windows\start menu\") {
        "Current user Start Menu"
    } else if value.contains(r"\desktop\microsoft edge.lnk") {
        "Current user Desktop"
    } else {
        "Shortcut"
    }
}

fn show_plain_dialog(message: &str) {
    let _ = nwg::simple_message(APP_TITLE, message);
}

fn get_hwnd(window: &nwg::Window) -> Option<HWND> {
    window
        .handle
        .hwnd()
        .map(|hwnd| HWND(hwnd as *mut core::ffi::c_void))
}

fn get_control_hwnd(handle: &nwg::ControlHandle) -> Option<HWND> {
    handle
        .hwnd()
        .map(|hwnd| HWND(hwnd as *mut core::ffi::c_void))
}


fn set_rich_text_box_format_rect(
    control: &nwg::RichTextBox,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
) {
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
        bottom: height as i32 - bottom,
    };

    unsafe {
        let _ = SendMessageW(
            hwnd,
            EM_SETRECT_MESSAGE,
            Some(WPARAM(0)),
            Some(LPARAM((&rect as *const RECT) as isize)),
        );
    }
}

fn set_window_enabled(window: &nwg::Window, enabled: bool) {
    let Some(hwnd) = get_hwnd(window) else {
        return;
    };

    unsafe {
        let _ = EnableWindow(hwnd, enabled);
    }
}

fn harden_window_chrome(window: &nwg::Window) {
    // Keep all app windows as fixed, icon-less dialog-style windows.
    let Some(hwnd) = get_hwnd(window) else {
        return;
    };

    unsafe {
        let _ = SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_SMALL as usize)),
            Some(LPARAM(0)),
        );

        let _ = SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_BIG as usize)),
            Some(LPARAM(0)),
        );

        let _ = SetClassLongPtrW(hwnd, GCLP_HICONSM, 0);
        let _ = SetClassLongPtrW(hwnd, GCLP_HICON, 0);

        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let _ = SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            ex_style | WS_EX_DLGMODALFRAME.0 as isize,
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
                | SWP_FRAMECHANGED,
        );
    }
}

fn center_window_on_cursor_monitor(window: &nwg::Window) {
    // Center popups on the monitor where the user is currently working.
    let Some(hwnd) = get_hwnd(window) else {
        return;
    };

    let (window_width, window_height) = window.size();

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
            dwFlags: 0,
        };

        if !GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
            return;
        }

        let work = monitor_info.rcWork;
        let work_width = work.right - work.left;
        let work_height = work.bottom - work.top;

        let x = work.left + (work_width - window_width as i32) / 2;
        let y = work.top + (work_height - window_height as i32) / 2;

        let _ = SetWindowPos(
            hwnd,
            None,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
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
) {
    // Each top-level window has its own handler so child button clicks are captured.
    let main_handle = app.window.handle.clone();
    let main_app = Rc::clone(app);

    let main_events = nwg::full_bind_event_handler(
        &main_handle,
        move |event, _event_data, handle| {
            main_app.process_event(event, handle);
        },
    );

    let custom_handle = app.custom_window.handle.clone();
    let custom_app = Rc::clone(app);

    let custom_events = nwg::full_bind_event_handler(
        &custom_handle,
        move |event, _event_data, handle| {
            custom_app.process_event(event, handle);
        },
    );

    let help_handle = app.help_window.handle.clone();
    let help_app = Rc::clone(app);

    let help_events = nwg::full_bind_event_handler(
        &help_handle,
        move |event, _event_data, handle| {
            help_app.process_event(event, handle);
        },
    );

    let status_handle = app.status_window.handle.clone();
    let status_app = Rc::clone(app);

    let status_events = nwg::full_bind_event_handler(
        &status_handle,
        move |event, _event_data, handle| {
            status_app.process_event(event, handle);
        },
    );

    (main_events, custom_events, help_events, status_events)
}

fn main() {
    nwg::init().expect("failed to initialize native-windows-gui");

    if let Err(error) = set_global_ui_font() {
        show_plain_dialog(&format!(
            "Failed to initialize Segoe UI font.\r\n\r\n{}",
            error
        ));
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
            show_plain_dialog(&format!(
                "Failed to build the user interface.\r\n\r\n{}",
                error
            ));
            return;
        }
    };

    let _handlers = bind_events(&app);

    nwg::dispatch_thread_events();
}
