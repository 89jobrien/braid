use regex::Regex;

use crate::rule::RedactionRule;

/// Replaces home directory paths with `~/...`.
pub struct HomePathRule {
    pattern: Regex,
}

impl HomePathRule {
    pub fn new() -> Self {
        Self {
            pattern: Regex::new(r"/(Users|home)/[A-Za-z0-9._-]+/").expect("valid regex"),
        }
    }
}

impl Default for HomePathRule {
    fn default() -> Self {
        Self::new()
    }
}

impl RedactionRule for HomePathRule {
    fn name(&self) -> &'static str {
        "home-path"
    }

    fn redact(&self, input: &str) -> String {
        self.pattern.replace_all(input, "~/").into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_macos_home_path() {
        let rule = HomePathRule::new();
        assert_eq!(rule.redact("/Users/joe/dev/project"), "~/dev/project");
    }

    #[test]
    fn redacts_linux_home_path() {
        let rule = HomePathRule::new();
        assert_eq!(rule.redact("/home/deploy/.config/app"), "~/.config/app");
    }

    #[test]
    fn leaves_non_home_paths_alone() {
        let rule = HomePathRule::new();
        let input = "/var/log/syslog";
        assert_eq!(rule.redact(input), input);
    }

    #[test]
    fn handles_multiple_paths() {
        let rule = HomePathRule::new();
        let input = "from /Users/alice/src to /home/bob/dst";
        assert_eq!(rule.redact(input), "from ~/src to ~/dst");
    }
}
