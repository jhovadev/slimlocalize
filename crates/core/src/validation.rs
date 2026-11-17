use std::collections::BTreeSet;

use crate::DomainError;
use crate::error::Result;

const MAX_NAME: usize = 120;
const MAX_SLUG: usize = 64;
const MAX_KEY: usize = 256;
const MAX_VALUE_CHARS: usize = 100_000;

/// Create a URL/file friendly slug. The transformation is deterministic and
/// ASCII-only by design; callers that need accents can provide an explicit slug.
#[must_use]
pub fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_dash = false;
    for ch in input.chars() {
        let mapped = match ch {
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            other => other,
        };
        if mapped.is_ascii_alphanumeric() {
            out.push(mapped.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("item");
    }
    out
}

pub fn validate_name(field: &str, name: &str) -> Result<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(DomainError::validation(field, "must not be empty"));
    }
    if trimmed.chars().count() > MAX_NAME {
        return Err(DomainError::validation(
            field,
            format!("must be at most {MAX_NAME} characters"),
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(DomainError::validation(
            field,
            "must not contain control characters",
        ));
    }
    Ok(())
}

pub fn validate_slug(slug: &str) -> Result<()> {
    if slug.is_empty() || slug.chars().count() > MAX_SLUG {
        return Err(DomainError::validation(
            "slug",
            format!("must contain between 1 and {MAX_SLUG} characters"),
        ));
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(DomainError::validation(
            "slug",
            "only lowercase ASCII letters, digits and dashes are allowed",
        ));
    }
    if slug.starts_with('-') || slug.ends_with('-') || slug.contains("--") {
        return Err(DomainError::validation(
            "slug",
            "must not start/end with a dash or contain consecutive dashes",
        ));
    }
    Ok(())
}

/// Validate and canonicalize a BCP-47-like locale code (`en`, `es-PE`,
/// `zh-Hant-TW`). The project intentionally supports a pragmatic subset rather
/// than the full Unicode locale grammar.
pub fn normalize_locale_code(input: &str) -> Result<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 35 {
        return Err(DomainError::validation(
            "locale code",
            "must contain between 1 and 35 characters",
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(DomainError::validation(
            "locale code",
            "only ASCII letters, digits, dashes and underscores are allowed",
        ));
    }
    let normalized_input = trimmed.replace('_', "-");
    let parts: Vec<&str> = normalized_input.split('-').collect();
    if parts.is_empty() || parts[0].len() < 2 || parts[0].len() > 3 {
        return Err(DomainError::validation(
            "locale code",
            "the language subtag must have 2 or 3 characters (for example en or es-PE)",
        ));
    }
    if !parts[0].chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(DomainError::validation(
            "locale code",
            "the language subtag must be alphabetic",
        ));
    }
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            return Err(DomainError::validation(
                "locale code",
                "empty subtags are not allowed",
            ));
        }
        if !part.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(DomainError::validation(
                "locale code",
                "subtags must be alphanumeric",
            ));
        }
        let valid_len = if index == 0 {
            part.len() == 2 || part.len() == 3
        } else if part.len() == 4 {
            part.chars().all(|c| c.is_ascii_alphabetic())
        } else if part.len() == 2 {
            part.chars().all(|c| c.is_ascii_alphabetic())
        } else if part.len() == 3 {
            part.chars().all(|c| c.is_ascii_digit())
        } else {
            (5..=8).contains(&part.len())
        };
        if !valid_len {
            return Err(DomainError::validation(
                "locale code",
                format!("invalid subtag {part:?}"),
            ));
        }
    }

    let mut out = String::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            out.push('-');
        }
        if index == 0 {
            out.push_str(&part.to_ascii_lowercase());
        } else if part.len() == 4 {
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                out.push(first.to_ascii_uppercase());
                out.extend(chars.map(|c| c.to_ascii_lowercase()));
            }
        } else if part.len() == 2 {
            out.push_str(&part.to_ascii_uppercase());
        } else {
            out.push_str(&part.to_ascii_lowercase());
        }
    }
    Ok(out)
}

pub fn validate_namespace(namespace: &str) -> Result<()> {
    if namespace.is_empty() || namespace.chars().count() > MAX_SLUG {
        return Err(DomainError::validation(
            "namespace",
            format!("must contain between 1 and {MAX_SLUG} characters"),
        ));
    }
    if !namespace
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        return Err(DomainError::validation(
            "namespace",
            "only ASCII letters, digits, underscores, dashes and dots are allowed",
        ));
    }
    if namespace.starts_with('.') || namespace.ends_with('.') || namespace.contains("..") {
        return Err(DomainError::validation(
            "namespace",
            "must not start/end with a dot or contain consecutive dots",
        ));
    }
    Ok(())
}

pub fn validate_key(key: &str) -> Result<()> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err(DomainError::validation("key", "must not be empty"));
    }
    if trimmed.chars().count() > MAX_KEY {
        return Err(DomainError::validation(
            "key",
            format!("must be at most {MAX_KEY} characters"),
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(DomainError::validation(
            "key",
            "must not contain control characters",
        ));
    }
    if trimmed.starts_with('.') || trimmed.ends_with('.') || trimmed.contains("..") {
        return Err(DomainError::validation(
            "key",
            "must not start/end with a dot or contain consecutive dots",
        ));
    }
    Ok(())
}

pub fn validate_translation_value(value: &str) -> Result<()> {
    if value.chars().count() > MAX_VALUE_CHARS {
        return Err(DomainError::validation(
            "value",
            format!("must be at most {MAX_VALUE_CHARS} characters"),
        ));
    }
    if value.contains('\0') {
        return Err(DomainError::validation("value", "must not contain NUL"));
    }
    Ok(())
}

pub fn validate_config_key(key: &str) -> Result<()> {
    let trimmed = key.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 128 {
        return Err(DomainError::validation(
            "config key",
            "must contain between 1 and 128 characters",
        ));
    }
    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else {
        return Err(DomainError::validation("config key", "must not be empty"));
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(DomainError::validation(
            "config key",
            "must start with a letter or underscore",
        ));
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(DomainError::validation(
            "config key",
            "only ASCII letters, digits and underscores are allowed",
        ));
    }
    Ok(())
}

/// Extract normalized placeholder names from a translation value.
///
/// Supported forms: `{name}`, `{{name}}`, `${name}`, `%s`, `%d`, `%1$s`.
/// ICU plural bodies are intentionally out of scope for F1; this is documented
/// as a basic placeholder validator, not a full message-format compiler.
#[must_use]
pub fn extract_placeholders(input: &str) -> BTreeSet<String> {
    let chars: Vec<char> = input.chars().collect();
    let mut found = BTreeSet::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            if i + 1 < chars.len() && chars[i + 1] == '{' {
                if let Some((name, next)) = parse_braced(&chars, i + 2, "}}") {
                    if is_placeholder_name(&name) {
                        found.insert(name);
                    }
                    i = next;
                    continue;
                }
            } else if let Some((name, next)) = parse_braced(&chars, i + 1, "}") {
                if is_placeholder_name(&name) {
                    found.insert(name);
                }
                i = next;
                continue;
            }
        }
        if chars[i] == '$'
            && i + 1 < chars.len()
            && chars[i + 1] == '{'
            && let Some((name, next)) = parse_braced(&chars, i + 2, "}")
        {
            if is_placeholder_name(&name) {
                found.insert(name);
            }
            i = next;
            continue;
        }
        if chars[i] == '%' {
            if i + 1 < chars.len() && chars[i + 1] == '%' {
                i += 2;
                continue;
            }
            let start = i;
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j < chars.len() && chars[j] == '$' {
                j += 1;
            }
            if j < chars.len() && chars[j].is_ascii_alphabetic() {
                let token: String = chars[start..=j].iter().collect();
                found.insert(token);
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    found
}

fn parse_braced(chars: &[char], start: usize, terminator: &str) -> Option<(String, usize)> {
    let mut text = String::new();
    let mut i = start;
    while i < chars.len() {
        if terminator == "}}" && i + 1 < chars.len() && chars[i] == '}' && chars[i + 1] == '}' {
            return Some((normalize_placeholder_name(&text), i + 2));
        }
        if terminator == "}" && chars[i] == '}' {
            return Some((normalize_placeholder_name(&text), i + 1));
        }
        text.push(chars[i]);
        i += 1;
    }
    None
}

fn normalize_placeholder_name(text: &str) -> String {
    let name = text.split(',').next().unwrap_or(text).trim();
    name.split(':').next().unwrap_or(name).trim().to_owned()
}

fn is_placeholder_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Compare placeholder sets. Returns `(missing_in_target, extra_in_target)`.
#[must_use]
pub fn compare_placeholders(source: &str, target: &str) -> (Vec<String>, Vec<String>) {
    let source_set = extract_placeholders(source);
    let target_set = extract_placeholders(target);
    let missing = source_set.difference(&target_set).cloned().collect();
    let extra = target_set.difference(&source_set).cloned().collect();
    (missing, extra)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_is_stable() {
        assert_eq!(slugify("Demo App"), "demo-app");
        assert_eq!(slugify("  Diseño   Público "), "diseno-publico");
        assert_eq!(slugify("---"), "item");
        assert_eq!(slugify("a--b"), "a-b");
    }

    #[test]
    fn locale_codes_are_canonicalized() {
        assert_eq!(normalize_locale_code("en-us").unwrap(), "en-US");
        assert_eq!(normalize_locale_code("ES_pe").unwrap(), "es-PE");
        assert_eq!(normalize_locale_code("zh-hant-tw").unwrap(), "zh-Hant-TW");
        assert!(normalize_locale_code("../../etc").is_err());
        assert!(normalize_locale_code("e").is_err());
        assert!(normalize_locale_code("en-").is_err());
    }

    #[test]
    fn placeholders_support_common_forms() {
        let values =
            extract_placeholders("Hi {name}, you have %d items, {{count}} total, ${currency}");
        assert!(values.contains("name"));
        assert!(values.contains("count"));
        assert!(values.contains("currency"));
        assert!(values.contains("%d"));
    }

    #[test]
    fn placeholder_comparison_detects_differences() {
        let (missing, extra) = compare_placeholders("Hello {name} {count}", "Hola {name}");
        assert_eq!(missing, vec!["count"]);
        assert!(extra.is_empty());
    }

    #[test]
    fn ignores_malformed_placeholders() {
        let values = extract_placeholders("{not a name} {ok} {");
        assert_eq!(values.len(), 1);
        assert!(values.contains("ok"));
    }

    #[test]
    fn validates_resource_names() {
        assert!(validate_key("auth.signIn").is_ok());
        assert!(validate_key("a..b").is_err());
        assert!(validate_namespace("common").is_ok());
        assert!(validate_namespace("a/b").is_err());
        assert!(validate_config_key("PUBLIC_API_URL").is_ok());
        assert!(validate_config_key("PUBLIC-API").is_err());
    }
}
