## About

A small Windows utility for applying Microsoft Edge shortcut settings.

Remove rounded corners, hide sign-in red dot, restore default shortcut settings, and apply custom flags.

## Features

* Rounded-corner workarounds
* Hide sign-in red dot
* Custom feature flags
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
3. Keep **Hide sign-in red dot** checked if needed.
4. Use **...** for Custom and **i** for Info.
5. Restart Edge.

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

Removes Edge custom flags managed by this tool from supported shortcuts, including standalone switches and `--enable-features` / `--disable-features`.

Other shortcut options, such as profile options, are preserved.

## Custom

Custom has three fields:

* **Standalone** accepts normal Edge command-line switches separated by spaces.
* **Enable features** accepts comma-separated feature names.
* **Disable features** accepts comma-separated feature names.

Standalone example:

```text
--force-dark-mode --disable-extensions --mute-audio
```

Enable feature examples:

```text
msForceNoRoundedCornerAndMargin,msDownloadsHub,ParallelDownloading
```

Disable feature examples:

```text
msShowSignInIndicator,msUndersideButton,MediaRouter
```

## Supported shortcuts

The tool updates existing shortcuts named:

```text
Microsoft Edge.lnk
```

The shortcut must exist in one of these supported locations:

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