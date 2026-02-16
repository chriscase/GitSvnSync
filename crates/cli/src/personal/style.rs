//! Shared styling utilities for the personal CLI.

use console::Style;

/// Create a success-styled string (green with checkmark).
pub fn success(msg: &str) -> String {
    let style = Style::new().green();
    format!("{} {}", style.apply_to("✓"), msg)
}

/// Create an error-styled string (red with cross).
pub fn error(msg: &str) -> String {
    let style = Style::new().red();
    format!("{} {}", style.apply_to("✗"), msg)
}

/// Create a warning-styled string (yellow).
pub fn warn(msg: &str) -> String {
    let style = Style::new().yellow();
    format!("{} {}", style.apply_to("⚠"), msg)
}

/// Create a header-styled string (bold, white).
pub fn header(msg: &str) -> String {
    let style = Style::new().bold();
    style.apply_to(msg).to_string()
}

/// Create a dim-styled string.
pub fn dim(msg: &str) -> String {
    let style = Style::new().dim();
    style.apply_to(msg).to_string()
}

/// Create a label for SVN→Git direction (blue).
pub fn svn_to_git() -> String {
    let style = Style::new().blue().bold();
    style.apply_to("SVN → Git").to_string()
}

/// Create a label for Git→SVN direction (green).
pub fn git_to_svn() -> String {
    let style = Style::new().green().bold();
    style.apply_to("Git → SVN").to_string()
}

/// Status indicator: running (green dot).
pub fn status_running() -> String {
    let style = Style::new().green();
    format!("{} Running", style.apply_to("●"))
}

/// Status indicator: stopped (dim dot).
pub fn status_stopped() -> String {
    let style = Style::new().dim();
    format!("{} Not running", style.apply_to("○"))
}
