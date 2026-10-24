//! Decisions that do not call Windows, kept apart so they can be tested.

/// What the foreground window is, as far as the taskbar is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Foreground {
    /// The Start menu or the search that opens from it.
    StartSurface,
    /// A taskbar itself (it takes the foreground while it is being clicked).
    Taskbar,
    Other,
}

const TASKBAR_CLASSES: [&str; 2] = ["Shell_TrayWnd", "Shell_SecondaryTrayWnd"];

// Confirmed for the Start menu; the notification center and quick settings do
// not match. TODO: check search (SearchHost.exe) on a real machine.
const START_PROCESSES: [&str; 3] = [
    "startmenuexperiencehost.exe",
    "searchhost.exe",
    "shellexperiencehost.exe",
];
const START_CLASSES: [&str; 2] = ["Windows.UI.Core.CoreWindow", "XamlExplorerHostIslandWindow"];

pub fn is_taskbar_class(class: &str) -> bool {
    TASKBAR_CLASSES.contains(&class)
}

/// `exe` is the file name of the foreground window's process, any case.
pub fn classify(class: &str, exe: &str) -> Foreground {
    if is_taskbar_class(class) {
        return Foreground::Taskbar;
    }
    let exe = exe.to_ascii_lowercase();
    if START_PROCESSES.contains(&exe.as_str()) && START_CLASSES.contains(&class) {
        return Foreground::StartSurface;
    }
    Foreground::Other
}

/// Tells a tap of the Win key (pressed and released with no other key in
/// between) from a shortcut such as Win+D.
#[derive(Debug, Default)]
pub struct WinTap {
    down: bool,
}

impl WinTap {
    /// Feeds one key event and returns true when it completes a tap.
    pub fn feed(&mut self, is_win: bool, is_down: bool) -> bool {
        match (is_win, is_down) {
            (true, true) => {
                self.down = true;
                false
            }
            (true, false) => std::mem::take(&mut self.down),
            (false, true) => {
                self.down = false;
                false
            }
            (false, false) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_surfaces_need_both_the_process_and_the_class() {
        assert_eq!(
            classify("Windows.UI.Core.CoreWindow", "StartMenuExperienceHost.exe"),
            Foreground::StartSurface
        );
        assert_eq!(
            classify("Windows.UI.Core.CoreWindow", "SearchHost.exe"),
            Foreground::StartSurface
        );
        // Other UWP apps use the same class.
        assert_eq!(classify("Windows.UI.Core.CoreWindow", "Calculator.exe"), Foreground::Other);
        assert_eq!(classify("Notepad", "StartMenuExperienceHost.exe"), Foreground::Other);
    }

    #[test]
    fn taskbars_are_their_own_kind() {
        assert_eq!(classify("Shell_TrayWnd", "explorer.exe"), Foreground::Taskbar);
        assert_eq!(classify("Shell_SecondaryTrayWnd", "explorer.exe"), Foreground::Taskbar);
        assert_eq!(classify("CabinetWClass", "explorer.exe"), Foreground::Other);
    }

    #[test]
    fn a_tap_is_down_then_up_with_nothing_between() {
        let mut t = WinTap::default();
        assert!(!t.feed(true, true));
        assert!(t.feed(true, false));
    }

    #[test]
    fn a_shortcut_is_not_a_tap() {
        let mut t = WinTap::default();
        t.feed(true, true);
        t.feed(false, true); // D goes down while Win is held
        t.feed(false, false);
        assert!(!t.feed(true, false));
    }

    #[test]
    fn key_repeat_of_win_is_still_a_tap() {
        let mut t = WinTap::default();
        t.feed(true, true);
        t.feed(true, true);
        assert!(t.feed(true, false));
    }

    #[test]
    fn an_up_without_a_down_is_not_a_tap() {
        let mut t = WinTap::default();
        assert!(!t.feed(true, false));
    }
}
