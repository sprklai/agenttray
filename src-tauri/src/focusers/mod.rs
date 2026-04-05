pub mod macos_app;
pub mod os_helpers;
pub mod unknown;
pub mod windows_native;
pub mod x11_generic;

pub fn dispatch(kind: &str, focus_id: &str, outer_id: &str) -> Result<(), String> {
    match kind {
        "x11_generic" => x11_generic::focus(focus_id, outer_id),
        "macos_app" => macos_app::focus(focus_id, outer_id),
        "windows_native" => windows_native::focus(focus_id, outer_id),
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
