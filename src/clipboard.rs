pub fn copy_to_clipboard(text: &str) -> bool {
    use std::io::Write;

    let candidates: &[(&str, &[&str])] = &[
        ("wl-copy", &[]),                        // Wayland
        ("xclip", &["-selection", "clipboard"]), // X11
        ("xsel", &["-bi"]),                      // X11 alt
        ("pbcopy", &[]),                         // macOS
    ];

    for (cmd, args) in candidates {
        if let Ok(mut child) = std::process::Command::new(cmd)
            .args(*args)
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().is_ok() {
                return true;
            }
        }
    }

    false
}
