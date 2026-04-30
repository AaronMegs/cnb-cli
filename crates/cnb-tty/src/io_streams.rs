//! Process-level IO streams abstraction (stdin/stdout/stderr) with TTY detection.

use is_terminal::IsTerminal;

use crate::color::ColorMode;

/// Snapshot of process IO stream properties at startup.
#[derive(Debug, Clone)]
pub struct IoStreams {
    pub stdin_is_tty: bool,
    pub stdout_is_tty: bool,
    pub stderr_is_tty: bool,
    pub color_mode: ColorMode,
    pub color_enabled: bool,
}

impl IoStreams {
    /// Build from the current process state and an explicit color preference.
    pub fn from_env(color_mode: ColorMode) -> Self {
        let stdout_is_tty = std::io::stdout().is_terminal();
        let stderr_is_tty = std::io::stderr().is_terminal();
        let stdin_is_tty = std::io::stdin().is_terminal();
        let color_enabled = color_mode.resolve(stdout_is_tty);
        Self {
            stdin_is_tty,
            stdout_is_tty,
            stderr_is_tty,
            color_mode,
            color_enabled,
        }
    }

    /// True if the program is being piped to/from another process.
    pub fn is_piped(&self) -> bool {
        !self.stdout_is_tty
    }
}

impl Default for IoStreams {
    fn default() -> Self {
        Self::from_env(ColorMode::default())
    }
}
