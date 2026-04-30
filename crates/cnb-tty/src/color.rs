//! Color mode resolution: respects `NO_COLOR`, `CLICOLOR`, and explicit user choice.

use std::env;

/// Color output mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

impl ColorMode {
    /// Resolve effective color mode based on `Self`, env, and TTY status.
    pub fn resolve(self, is_tty: bool) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => {
                // NO_COLOR (https://no-color.org/) — set, non-empty disables color.
                if env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
                    return false;
                }
                // CLICOLOR=0 disables, CLICOLOR_FORCE forces.
                if env::var("CLICOLOR_FORCE").map(|v| v != "0").unwrap_or(false) {
                    return true;
                }
                if env::var("CLICOLOR").map(|v| v == "0").unwrap_or(false) {
                    return false;
                }
                is_tty
            }
        }
    }
}

impl Default for ColorMode {
    fn default() -> Self {
        Self::Auto
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_clean_env<T>(f: impl FnOnce() -> T) -> T {
        let keys = ["NO_COLOR", "CLICOLOR", "CLICOLOR_FORCE"];
        let saved: Vec<_> = keys.iter().map(|k| (*k, env::var_os(k))).collect();
        for k in &keys {
            env::remove_var(k);
        }
        let r = f();
        for (k, v) in saved {
            match v {
                Some(v) => env::set_var(k, v),
                None => env::remove_var(k),
            }
        }
        r
    }

    #[test]
    fn always_and_never_are_unconditional() {
        assert!(ColorMode::Always.resolve(false));
        assert!(!ColorMode::Never.resolve(true));
    }

    #[test]
    fn auto_follows_tty_when_no_env() {
        with_clean_env(|| {
            assert!(ColorMode::Auto.resolve(true));
            assert!(!ColorMode::Auto.resolve(false));
        });
    }

    #[test]
    fn no_color_disables() {
        with_clean_env(|| {
            env::set_var("NO_COLOR", "1");
            assert!(!ColorMode::Auto.resolve(true));
            env::remove_var("NO_COLOR");
        });
    }

    #[test]
    fn clicolor_force_enables_even_off_tty() {
        with_clean_env(|| {
            env::set_var("CLICOLOR_FORCE", "1");
            assert!(ColorMode::Auto.resolve(false));
            env::remove_var("CLICOLOR_FORCE");
        });
    }
}
