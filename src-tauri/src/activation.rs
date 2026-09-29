/// Empty activation variables must not mask the legacy fallback.
pub fn token(primary: Option<String>, fallback: Option<String>) -> Option<String> {
    primary.filter(|s| !s.is_empty()).or_else(|| fallback.filter(|s| !s.is_empty()))
}

pub fn environment_token() -> Option<String> {
    token(std::env::var("XDG_ACTIVATION_TOKEN").ok(), std::env::var("DESKTOP_STARTUP_ID").ok())
}

pub fn argument_token(args: &[String]) -> Option<&str> {
    args.iter().filter_map(|arg| arg.strip_prefix("--xdg-token=")).find(|t| !t.is_empty())
}

pub fn trace(message: &str) {
    if std::env::var_os("CLIPBOARD_DEBUG_FOCUS").is_some() {
        eprintln!("[clipboard focus] {message}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_primary_uses_fallback() {
        assert_eq!(token(Some("".into()), Some("legacy".into())).as_deref(), Some("legacy"));
        assert_eq!(token(Some("new".into()), Some("legacy".into())).as_deref(), Some("new"));
        assert_eq!(token(None, Some("".into())), None);
    }
    #[test]
    fn ignores_empty_argument_tokens() {
        let args = vec!["clipboard".into(), "--xdg-token=".into(), "--xdg-token=valid".into()];
        assert_eq!(argument_token(&args), Some("valid"));
    }
}
