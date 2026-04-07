pub mod macos_app;
pub mod os_helpers;
pub mod unknown;
pub mod windows_native;
pub mod x11_generic;

// Cross-platform focusers (CLI-based)
pub mod kitty;
pub mod neovim;
pub mod screen;
pub mod tmux;
pub mod vscode;
pub mod wezterm;
pub mod zellij;

// IDE focusers
pub mod jetbrains;

pub fn dispatch(kind: &str, focus_id: &str, outer_id: &str) -> Result<(), String> {
    match kind {
        // Platform-specific
        "x11_generic" => x11_generic::focus(focus_id, outer_id),
        "macos_app" => macos_app::focus(focus_id, outer_id),
        "windows_native" => windows_native::focus(focus_id, outer_id),
        // Cross-platform (CLI-based)
        "kitty" => kitty::focus(focus_id, outer_id),
        "tmux" => tmux::focus(focus_id, outer_id),
        "screen" => screen::focus(focus_id, outer_id),
        "wezterm" => wezterm::focus(focus_id, outer_id),
        "zellij" => zellij::focus(focus_id, outer_id),
        "neovim" => neovim::focus(focus_id, outer_id),
        // IDE terminals
        "vscode" => vscode::focus(focus_id, outer_id),
        "jetbrains" => jetbrains::focus(focus_id, outer_id),
        _ => unknown::focus(focus_id, outer_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_unknown_kind_returns_err() {
        assert!(dispatch("future_terminal_xyz", "some_id", "").is_err());
    }

    #[test]
    fn dispatch_empty_kind_returns_err() {
        assert!(dispatch("", "", "").is_err());
    }

    #[test]
    fn dispatch_x11_empty_focus_id_returns_ok() {
        // x11_generic with empty focus_id is a no-op, not an error
        assert!(dispatch("x11_generic", "", "").is_ok());
    }
}
