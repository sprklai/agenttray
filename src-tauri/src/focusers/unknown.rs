pub fn focus(_focus_id: &str, _outer_id: &str) -> Result<(), String> {
    // No-op fallback for unknown terminal types
    Ok(())
}
