# ytaskbary

[日本語](README.ja.md)

A Windows 11 tool that keeps the taskbar hidden and shows it only while the Start menu is in front. It stays in the system tray. With Windows' own "Automatically hide the taskbar" alone, the taskbar comes back when the mouse pointer reaches the edge of the screen; ytaskbary keeps it hidden then too.

## Features

- Hides the taskbars of all monitors
- By default, shows them while the Start menu is in front, and hides them again when you switch to another window
- By default, does not watch key presses and runs nothing at intervals, so it does not slow down key input while a game is in front
- Menu in the system tray:
  - Enabled: clear it to stop hiding the taskbar
  - Start at sign-in: start ytaskbary when you sign in
  - Exit: quit, and show the taskbar again

## Install

Install with [Scoop](https://scoop.sh/), or download `ytaskbary.exe` from [Releases](https://github.com/ycookiey/ytaskbary/releases).

```powershell
scoop bucket add yscoopy https://github.com/ycookiey/yscoopy
scoop install yscoopy/ytaskbary
```

## Usage

1. In Windows Settings, turn on "Automatically hide the taskbar" (Personalization > Taskbar > Taskbar behaviors), so that maximized windows also use the space of the taskbar
2. Run `ytaskbary.exe`. The taskbars hide
3. To open the menu, open the Start menu so that the taskbar shows, then click the ytaskbary icon in the system tray. It may be among the hidden icons (^)

If the taskbar does not show when the Start menu opens, exit ytaskbary and start it with `--keyboard-hook`. It then shows the taskbars when you press and release the Win key alone, and not when the Start menu opens in other ways. To watch key presses it uses a low-level keyboard hook, and while the hook is set, Windows waits for ytaskbary on every key press. So the hook is removed while a game is in front: an app whose executable is in the list of games that Windows keeps for the Xbox Game Bar. "Start at sign-in" keeps the option that ytaskbary was started with when you turned it on.

## Notes

- Tested on Windows 11 only, with the taskbar on one monitor
- If ytaskbary ends without "Exit" (for example from Task Manager), the taskbar stays hidden. Start ytaskbary again and choose "Exit"
- Log file: `%LOCALAPPDATA%\ytaskbary\ytaskbary.log`

## Uninstall

1. In the menu, clear "Start at sign-in" if it is checked
2. Choose "Exit" in the menu
3. Run `scoop uninstall ytaskbary`, or delete `ytaskbary.exe` if you downloaded it from Releases
4. Delete the folder `%LOCALAPPDATA%\ytaskbary` that holds the log

## Build

Requires Rust with the MSVC toolchain.

```powershell
cargo build --release
.\target\release\ytaskbary.exe
```

## License

[MIT License](LICENSE)
