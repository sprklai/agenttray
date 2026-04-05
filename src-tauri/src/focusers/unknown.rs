pub fn focus(_focus_id: &str, _outer_id: &str) -> Result<(), String> {
    Err("Focus not supported for this terminal type".to_string())
}
