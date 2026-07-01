## About

A small Windows utility for applying Microsoft Edge shortcut settings.

## Features

* Rounded-corner workarounds
* Hide sign-in red dot
* Restore sidebar
* Disable extensions
* Custom feature flags
* Choose which shortcut names are updated
* Apply preset when Edge is opened by another app
* Native Windows app

## Before using

Disable **Startup Boost** in Microsoft Edge first:

```text
edge://settings/system/manageSystem
```

Then fully close and restart Edge after applying changes.

## Usage

1. Run `EdgeShortcutTool.exe` with Administrator privilege.
2. Choose either:

   * **Old workaround**
   * **New workaround**
   * **Restore default**
3. Keep **Hide sign-in red dot**, **Restore sidebar**, or **Disable extensions** checked if needed.
4. Use **Advanced** to choose shortcut targets or enable support for links opened from other apps.
5. Use **...** for Custom, **?** for Help, and **i** for Info.
6. Restart Edge.

The tool remembers your selected options for the next time you open it.

## Presets

### Old workaround

```text
--disable-features="msFeatureGroupNewLookAndFeelHoldout"
```

With **Hide sign-in red dot** checked:

```text
--disable-features="msShowSignInIndicator,msFeatureGroupNewLookAndFeelHoldout"
```

### New workaround

```text
--enable-features="msForceNoRoundedCornerAndMargin"
```

With **Hide sign-in red dot** checked:

```text
--enable-features="msForceNoRoundedCornerAndMargin" --disable-features="msShowSignInIndicator"
```

### Restore default

Clears command-line flags managed by this tool and restores normal Edge launch behavior.

This includes standalone switches, `--enable-features` and `--disable-features`.

## Advanced

Advanced has two sections.

**Shortcut targets** chooses which shortcut names are updated:

```text
Microsoft Edge
Microsoft Edge Beta
Microsoft Edge Dev
Microsoft Edge Canary
```

Microsoft Edge is selected by default.

**External links** applies the same flags when Edge is opened by another app, such as Discord, Teams, Terminal, or other apps that open web links directly.

This is useful when the shortcut fix works, but rounded corners come back after opening a link from another app.

Use **Restore default** to restore normal Edge launch behavior managed by this tool.

## Custom

Custom has three fields:

* **Standalone** accepts normal command-line switches separated by spaces.
* **Enable features** accepts comma-separated feature names.
* **Disable features** accepts comma-separated feature names.

Standalone example:

```text
--disable-extensions --force-dark-mode --mute-audio
```

Enable feature examples:

```text
msForceNoRoundedCornerAndMargin,msUndersideButton,ParallelDownloading
```

Disable feature examples:

```text
msShowSignInIndicator,msUndersideButton,MediaRouter
```

Custom flags also use the **External links** setting from Advanced.

## Supported shortcuts

The tool updates existing shortcuts named:

```text
Microsoft Edge.lnk
```

Or optionally:

```text
Microsoft Edge Beta.lnk
Microsoft Edge Dev.lnk
Microsoft Edge Canary.lnk
```

Microsoft Edge.lnk must exist in one of these common locations:

```text
%Public%\Desktop\Microsoft Edge.lnk
%UserProfile%\Desktop\Microsoft Edge.lnk
%ProgramData%\Microsoft\Windows\Start Menu\Programs\Microsoft Edge.lnk
%AppData%\Microsoft\Windows\Start Menu\Programs\Microsoft Edge.lnk
%AppData%\Microsoft\Internet Explorer\Quick Launch\Microsoft Edge.lnk
%AppData%\Microsoft\Internet Explorer\Quick Launch\User Pinned\StartMenu\Microsoft Edge.lnk
%AppData%\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\Microsoft Edge.lnk
```

Missing shortcuts are skipped silently.

## Limitations

* The shortcut name must be `Microsoft Edge.lnk`.
* Custom shortcut locations may not be detected.
* It does not create new shortcuts.

## Build

```bat
cargo build --release
```

Output:

```text
target\release\EdgeShortcutTool.exe
```

## License

MIT License
