use std::collections::HashMap;

use anyhow::{Context, Result};

/// Parse a CSV document into rows, handling BOM, quotes, embedded commas and
/// newlines. Blank rows are removed.
pub fn parse_csv(text: &str) -> Result<Vec<Vec<String>>> {
    let input = text.trim_start_matches('\u{feff}');
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(input.as_bytes());

    let mut rows = Vec::new();
    for row in reader.records() {
        let record = row.context("invalid CSV row")?;
        let values: Vec<String> = record
            .iter()
            .map(|value| value.trim().to_string())
            .collect();
        if values.iter().any(|value| !value.is_empty()) {
            rows.push(values);
        }
    }
    Ok(rows)
}

/// Parse CSV rows into maps keyed by the header row.
pub fn parse_csv_objects(text: &str) -> Result<Vec<HashMap<String, String>>> {
    let mut rows = parse_csv(text)?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let header = rows.remove(0);
    let keys: Vec<String> = header
        .into_iter()
        .map(|key| key.trim().to_string())
        .collect();

    Ok(rows
        .into_iter()
        .map(|row| {
            let mut object = HashMap::with_capacity(keys.len());
            for (index, key) in keys.iter().enumerate() {
                object.insert(
                    key.clone(),
                    row.get(index)
                        .cloned()
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                );
            }
            object
        })
        .collect())
}

/// Parse a number, treating blanks and "-" as absent.
pub fn num(value: Option<&String>) -> Option<f64> {
    let value = value?.trim();
    if value.is_empty() || value == "-" {
        return None;
    }
    value.parse::<f64>().ok()
}

pub fn text<'a>(row: &'a HashMap<String, String>, key: &str) -> &'a str {
    row.get(key).map(String::as_str).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_handles_bom_quotes_and_newlines() {
        let input = "\u{feff}name,note,value\n\"Central, Hong Kong\",\"line1\nline2\",3\n";
        let rows = parse_csv(input).unwrap();
        assert_eq!(
            rows,
            vec![
                vec!["name", "note", "value"],
                vec!["Central, Hong Kong", "line1\nline2", "3"]
            ]
        );
        let objects = parse_csv_objects(input).unwrap();
        assert_eq!(objects[0]["name"], "Central, Hong Kong");
        assert_eq!(objects[0]["note"], "line1\nline2");
    }

    #[test]
    fn csv_handles_escaped_quotes() {
        let rows = parse_csv("a,b\n\"say \"\"hi\"\"\",2\n").unwrap();
        assert_eq!(rows[1], vec!["say \"hi\"", "2"]);
    }
}
