use chrono::NaiveDate;

/// Convert a title to kebab-case for use in folder names.
///
/// - Trims leading/trailing whitespace
/// - Replaces spaces/underscores with `-`
/// - Removes non-alphanumeric characters (except `-`)
/// - Collapses repeated `-`
/// - Lowercases
pub fn kebab_case(title: &str) -> String {
    let mut result = String::new();
    let mut last_dash = false;
    for ch in title.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if ch.is_whitespace() || ch == '_' || ch == '-' {
            if !last_dash && !result.is_empty() {
                result.push('-');
                last_dash = true;
            }
        }
    }
    if result.ends_with('-') {
        result.pop();
    }
    result
}

/// Format a date as YYYY.MM.DD for folder naming.
pub fn pretty_date(date: &NaiveDate) -> String {
    date.format("%Y.%m.%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kebab_case() {
        assert_eq!(kebab_case("Hello World"), "hello-world");
        assert_eq!(kebab_case("  Foo_Bar  "), "foo-bar");
        assert_eq!(kebab_case("Test--Multiple---Dashes"), "test-multiple-dashes");
        assert_eq!(kebab_case("Special!@#Characters"), "specialcharacters");
    }
}
