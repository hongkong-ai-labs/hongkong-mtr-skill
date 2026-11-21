use std::sync::OnceLock;

use regex::Regex;

fn tag_re() -> &'static Regex {
    static TAG: OnceLock<Regex> = OnceLock::new();
    TAG.get_or_init(|| Regex::new(r"(?s)<[^>]+>").expect("valid tag regex"))
}

fn newline_re() -> &'static Regex {
    static NEWLINE: OnceLock<Regex> = OnceLock::new();
    NEWLINE.get_or_init(|| Regex::new(r"\n{3,}").expect("valid newline regex"))
}

fn space_re() -> &'static Regex {
    static SPACE: OnceLock<Regex> = OnceLock::new();
    SPACE.get_or_init(|| Regex::new(r"[ \t]+").expect("valid space regex"))
}

fn pipe_space_re() -> &'static Regex {
    static PIPE: OnceLock<Regex> = OnceLock::new();
    PIPE.get_or_init(|| Regex::new(r" *\| *\n").expect("valid pipe regex"))
}

fn named_entity(name: &str) -> Option<char> {
    match name.to_ascii_lowercase().as_str() {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        "ndash" => Some('–'),
        "mdash" => Some('—'),
        "hellip" => Some('…'),
        "rsquo" => Some('’'),
        "lsquo" => Some('‘'),
        "ldquo" => Some('“'),
        "rdquo" => Some('”'),
        "middot" => Some('·'),
        _ => None,
    }
}

fn code_point(value: u32) -> String {
    char::from_u32(value)
        .map(|character| character.to_string())
        .unwrap_or_default()
}

pub fn decode_entities(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(start) = remaining.find('&') {
        output.push_str(&remaining[..start]);
        let after = &remaining[start + 1..];
        let Some(end) = after.find(';') else {
            output.push('&');
            remaining = after;
            continue;
        };
        let entity = &after[..end];
        let decoded = if let Some(hex) = entity
            .strip_prefix("#x")
            .or_else(|| entity.strip_prefix("#X"))
        {
            u32::from_str_radix(hex, 16).ok().map(code_point)
        } else if let Some(decimal) = entity.strip_prefix('#') {
            decimal.parse::<u32>().ok().map(code_point)
        } else {
            named_entity(entity).map(|character| character.to_string())
        };

        match decoded {
            Some(value) => {
                output.push_str(&value);
                remaining = &after[end + 1..];
            }
            None => {
                output.push('&');
                remaining = after;
            }
        }
    }
    output.push_str(remaining);
    output
}

/// Convert MTR's alert HTML into readable text while keeping paragraph, list
/// and table structure useful to an LLM.
pub fn html_to_text(html: Option<&str>) -> String {
    let Some(html) = html else {
        return String::new();
    };
    if html.is_empty() {
        return String::new();
    }

    let mut text = html.to_string();
    for (pattern, replacement) in [
        (r"(?i)<\s*br\s*/?>", "\n"),
        (r"(?i)<\s*/\s*p\s*>", "\n\n"),
        (r"(?i)<\s*/\s*div\s*>", "\n"),
        (r"(?i)<\s*li[^>]*>", "\n- "),
        (r"(?i)<\s*/\s*li\s*>", ""),
        (r"(?i)<\s*/\s*tr\s*>", "\n"),
        (r"(?i)<\s*/\s*(td|th)\s*>", " | "),
        (r"(?i)<\s*table[^>]*>", "\n"),
        (r"(?i)<\s*/\s*table\s*>", "\n"),
    ] {
        let regex = Regex::new(pattern).expect("valid HTML replacement regex");
        text = regex.replace_all(&text, replacement).into_owned();
    }

    text = tag_re().replace_all(&text, "").into_owned();
    text = decode_entities(&text);
    text = space_re().replace_all(&text, " ").into_owned();
    text = pipe_space_re().replace_all(&text, "\n").into_owned();
    text = newline_re().replace_all(&text, "\n\n").into_owned();

    text.lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

pub fn one_line(input: Option<&str>) -> String {
    html_to_text(input)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn first_number(input: &str) -> Option<i64> {
    let mut digits = String::new();
    let mut started = false;
    for character in input.chars() {
        if character.is_ascii_digit() {
            started = true;
            digits.push(character);
        } else if started {
            break;
        }
    }
    digits.parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_becomes_readable_text_and_tables() {
        let text = html_to_text(Some(
            "<p>Service ends early&nbsp;today.</p><table><tr><td>From Tung Chung</td><td>23:02</td></tr></table><ul><li>Use bus</li></ul>",
        ));
        assert!(text.contains("Service ends early today."));
        assert!(text.contains("From Tung Chung | 23:02"));
        assert!(text.contains("- Use bus"));
    }

    #[test]
    fn decodes_numeric_and_named_entities() {
        assert_eq!(
            decode_entities("A &amp; B &#x4E2D; &#25991;"),
            "A & B 中 文"
        );
    }
}
