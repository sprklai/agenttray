/// Focus a Neovim terminal buffer via its RPC socket.
/// `focus_id` is the NVIM socket path (e.g. "/tmp/nvimXXXXXX/0").

pub fn focus(focus_id: &str, _outer_id: &str) -> Result<(), String> {
    if focus_id.is_empty() {
        return Ok(());
    }

    use super::os_helpers::spawn_silent;

    // Send <C-\><C-n> to return to normal mode, making the terminal visible
    // This won't switch to the exact buffer but signals the user.
    spawn_silent(
        "nvim",
        &["--server", focus_id, "--remote-send", "<C-\\><C-n>"],
    )
}
