#[derive(Debug, PartialEq)]
pub struct ParsedProgress {
    pub percent: f64,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub speed: Option<String>,
    pub eta: Option<String>,
}

fn optional_text(value: Option<&&str>) -> Option<String> {
    value
        .map(|v| v.trim())
        .filter(|v| !v.is_empty() && *v != "NA" && *v != "N/A")
        .map(str::to_string)
}

fn optional_number(value: Option<&&str>) -> Option<u64> {
    value
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(|v| v.max(0.0) as u64)
}

pub fn parse_progress(line: &str) -> Option<ParsedProgress> {
    let fields: Vec<_> = line.trim().split('|').collect();
    if fields.first()? != &"OF_PROGRESS" {
        return None;
    }
    let percent = fields
        .get(1)?
        .trim()
        .trim_end_matches('%')
        .trim()
        .parse::<f64>()
        .ok()?;
    Some(ParsedProgress {
        percent: percent.clamp(0.0, 100.0),
        downloaded_bytes: optional_number(fields.get(2)),
        total_bytes: optional_number(fields.get(3)),
        speed: optional_text(fields.get(4)),
        eta: optional_text(fields.get(5)),
    })
}

pub fn parse_output(line: &str) -> Option<String> {
    line.trim()
        .strip_prefix("OF_FILE|")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub fn parse_item(line: &str) -> Option<String> {
    line.trim()
        .strip_prefix("OF_ITEM|")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_machine_progress() {
        let value = parse_progress("OF_PROGRESS| 42.5%|1024|2048|2.1MiB/s|00:04").unwrap();
        assert_eq!(value.percent, 42.5);
        assert_eq!(value.downloaded_bytes, Some(1024));
        assert_eq!(value.eta.as_deref(), Some("00:04"));
    }
    #[test]
    fn ignores_human_output() {
        assert!(parse_progress("[download] 10%").is_none());
    }
    #[test]
    fn parses_file() {
        assert_eq!(
            parse_output("OF_FILE|C:\\a.mp4").as_deref(),
            Some("C:\\a.mp4")
        );
    }
}
