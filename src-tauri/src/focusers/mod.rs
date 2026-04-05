pub mod os_helpers;
pub mod unknown;
pub mod x11_generic;

pub fn dispatch(kind: &str, focus_id: &str, outer_id: &str) -> Result<(), String> {
    match kind {
        "x11_generic" => x11_generic::focus(focus_id, outer_id),
        _ => unknown::focus(focus_id, outer_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_unknown_kind_returns_ok() {
        assert!(dispatch("future_terminal_xyz", "some_id", "").is_ok());
    }

    #[test]
    fn dispatch_empty_kind_returns_ok() {
        assert!(dispatch("", "", "").is_ok());
    }

    #[test]
    fn dispatch_empty_focus_id_returns_ok() {
        for kind in &["x11_generic", "unknown"] {
            assert!(
                dispatch(kind, "", "").is_ok(),
                "dispatch({kind}) panicked on empty focus_id"
            );
        }
    }
}
