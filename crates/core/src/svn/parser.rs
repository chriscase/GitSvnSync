//! Parsers for SVN XML output.

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::errors::SvnError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvnInfo {
    pub root_url: String,
    pub uuid: String,
    pub latest_rev: i64,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvnLogEntry {
    pub revision: i64,
    pub author: String,
    pub date: String,
    pub message: String,
    pub changed_paths: Vec<SvnChangedPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvnChangedPath {
    pub action: String,
    pub path: String,
    pub copy_from_path: Option<String>,
    pub copy_from_rev: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvnDiffEntry {
    pub kind: String,
    pub props_changed: bool,
    pub path: String,
    pub item: String,
}

pub fn parse_svn_info(xml: &str) -> Result<SvnInfo, SvnError> {
    debug!("parsing svn info XML ({} bytes)", xml.len());
    let url = extract_tag_content(xml, "url")
        .ok_or_else(|| SvnError::XmlParseError("missing <url> in svn info".into()))?;
    let root_url = extract_tag_content(xml, "root")
        .ok_or_else(|| SvnError::XmlParseError("missing <root> in svn info".into()))?;
    let uuid = extract_tag_content(xml, "uuid")
        .ok_or_else(|| SvnError::XmlParseError("missing <uuid> in svn info".into()))?;
    let latest_rev = extract_attribute(xml, "entry", "revision")
        .or_else(|| extract_attribute(xml, "commit", "revision"))
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| SvnError::XmlParseError("missing revision in svn info".into()))?;
    Ok(SvnInfo {
        root_url,
        uuid,
        latest_rev,
        url,
    })
}

pub fn parse_svn_log(xml: &str) -> Result<Vec<SvnLogEntry>, SvnError> {
    debug!("parsing svn log XML ({} bytes)", xml.len());
    let mut entries = Vec::new();
    let parts: Vec<&str> = xml.split("<logentry").collect();
    for part in parts.iter().skip(1) {
        let entry_xml = match part.find("</logentry>") {
            Some(pos) => &part[..pos],
            None => part,
        };
        let revision = extract_attribute_from_fragment(entry_xml, "revision")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let author = extract_tag_content(entry_xml, "author").unwrap_or_default();
        let date = extract_tag_content(entry_xml, "date").unwrap_or_default();
        let message = extract_tag_content(entry_xml, "msg").unwrap_or_default();
        let changed_paths = parse_changed_paths(entry_xml);
        entries.push(SvnLogEntry {
            revision,
            author,
            date,
            message,
            changed_paths,
        });
    }
    debug!(count = entries.len(), "parsed svn log entries");
    Ok(entries)
}

pub fn parse_svn_diff_summarize(xml: &str) -> Result<Vec<SvnDiffEntry>, SvnError> {
    debug!("parsing svn diff --summarize XML ({} bytes)", xml.len());
    let mut entries = Vec::new();
    let parts: Vec<&str> = xml.split("<path ").collect();
    for part in parts.iter().skip(1) {
        let fragment = match part.find("</path>") {
            Some(pos) => &part[..pos],
            None => continue,
        };
        let item = extract_attribute_from_fragment(fragment, "item").unwrap_or_default();
        let kind_attr = extract_attribute_from_fragment(fragment, "kind").unwrap_or_default();
        let props = extract_attribute_from_fragment(fragment, "props").unwrap_or_default();
        let path = match fragment.find('>') {
            Some(pos) => fragment[pos + 1..].trim().to_string(),
            None => String::new(),
        };
        entries.push(SvnDiffEntry {
            kind: item,
            props_changed: props != "none",
            path,
            item: kind_attr,
        });
    }
    debug!(count = entries.len(), "parsed svn diff entries");
    Ok(entries)
}

fn extract_tag_content(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let start_pos = xml.find(&open)?;
    let after_open = &xml[start_pos + open.len()..];
    let content_start = after_open.find('>')? + 1;
    let content = &after_open[content_start..];
    let end_pos = content.find(&close)?;
    Some(content[..end_pos].trim().to_string())
}

fn extract_attribute(xml: &str, tag: &str, attr: &str) -> Option<String> {
    let open = format!("<{}", tag);
    let start_pos = xml.find(&open)?;
    let after_tag = &xml[start_pos + open.len()..];
    let tag_end = after_tag.find('>')?;
    extract_attr_from_str(&after_tag[..tag_end], attr)
}

fn extract_attribute_from_fragment(fragment: &str, attr: &str) -> Option<String> {
    extract_attr_from_str(fragment, attr)
}

fn extract_attr_from_str(s: &str, attr: &str) -> Option<String> {
    let pattern_dq = format!("{}=\"", attr);
    if let Some(pos) = s.find(&pattern_dq) {
        let after = &s[pos + pattern_dq.len()..];
        let end = after.find('"')?;
        return Some(after[..end].to_string());
    }
    let pattern_sq = format!("{}='", attr);
    if let Some(pos) = s.find(&pattern_sq) {
        let after = &s[pos + pattern_sq.len()..];
        let end = after.find('\'')?;
        return Some(after[..end].to_string());
    }
    None
}

fn parse_changed_paths(entry_xml: &str) -> Vec<SvnChangedPath> {
    let mut paths = Vec::new();
    let paths_block = match entry_xml.find("<paths>") {
        Some(start) => {
            let rest = &entry_xml[start..];
            match rest.find("</paths>") {
                Some(end) => &rest[..end],
                None => return paths,
            }
        }
        None => return paths,
    };
    let parts: Vec<&str> = paths_block.split("<path").collect();
    for part in parts.iter().skip(1) {
        let fragment = match part.find("</path>") {
            Some(pos) => &part[..pos],
            None => continue,
        };
        let action = extract_attribute_from_fragment(fragment, "action").unwrap_or_default();
        let copy_from_path = extract_attribute_from_fragment(fragment, "copyfrom-path");
        let copy_from_rev = extract_attribute_from_fragment(fragment, "copyfrom-rev")
            .and_then(|s| s.parse::<i64>().ok());
        let path = match fragment.find('>') {
            Some(pos) => fragment[pos + 1..].trim().to_string(),
            None => String::new(),
        };
        paths.push(SvnChangedPath {
            action,
            path,
            copy_from_path,
            copy_from_rev,
        });
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_svn_info() {
        let xml = r#"<info><entry kind="dir" path="." revision="1234">
<url>https://svn.example.com/repo/trunk</url>
<repository><root>https://svn.example.com/repo</root>
<uuid>a1b2c3d4</uuid></repository>
<commit revision="1234"></commit></entry></info>"#;
        let info = parse_svn_info(xml).unwrap();
        assert_eq!(info.latest_rev, 1234);
    }

    #[test]
    fn test_parse_svn_log() {
        let xml = r#"<log><logentry revision="100"><author>alice</author><date>2025-01-10</date>
<paths><path action="M" kind="file">/trunk/main.rs</path></paths><msg>fix</msg></logentry></log>"#;
        let entries = parse_svn_log(xml).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].revision, 100);
    }
}
