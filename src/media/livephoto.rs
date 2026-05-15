use std::path::Path;
use std::io::Read;
use anyhow::Result;

/// Check if a JPEG file is an Android Motion Photo by scanning its XMP segment.
pub fn is_motion_photo(path: &Path) -> Result<bool> {
    let xmp = match read_jpeg_xmp(path) {
        Ok(Some(x)) => x,
        _ => return Ok(false),
    };
    Ok(xmp_has_motion_photo(&xmp))
}

/// Read the iOS Live Photo ContentIdentifier from the XMP segment (apple-fi:Identifier).
pub fn read_content_identifier_xmp(path: &Path) -> Option<String> {
    let xmp = read_jpeg_xmp(path).ok()??;
    extract_xmp_attr(&xmp, "apple-fi:Identifier")
        .or_else(|| extract_xmp_attr(&xmp, "Identifier"))
}

/// Returns (is_motion_photo, content_identifier_xmp) from a single file read.
/// Use this instead of calling is_motion_photo + read_content_identifier_xmp separately
/// to avoid reading the file twice.
pub fn read_xmp_data(path: &Path) -> (bool, Option<String>) {
    match read_jpeg_xmp(path) {
        Ok(Some(xmp)) => {
            let is_motion = xmp_has_motion_photo(&xmp);
            let id = extract_xmp_attr(&xmp, "apple-fi:Identifier")
                .or_else(|| extract_xmp_attr(&xmp, "Identifier"));
            (is_motion, id)
        }
        _ => (false, None),
    }
}

// ── XMP helpers ──────────────────────────────────────────────────────────────

/// Read the XMP packet from a JPEG file (APP1 segment with Adobe XMP namespace).
/// Only reads the first 512 KB — XMP is always in APP1 markers near the start of the file.
/// This prevents loading entire large files (e.g. Android Motion Photos with embedded video)
/// into memory, which would cause system memory exhaustion and Windows freeze.
fn read_jpeg_xmp(path: &Path) -> Result<Option<String>> {
    let file = std::fs::File::open(path)?;
    let mut buf = Vec::with_capacity(65536);
    file.take(512 * 1024).read_to_end(&mut buf)?;

    const XMP_HEADER: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

    let mut i = 0usize;
    while i + 4 < buf.len() {
        // Look for APP1 marker (0xFF 0xE1)
        if buf[i] == 0xFF && buf[i + 1] == 0xE1 {
            let seg_len = u16::from_be_bytes([buf[i + 2], buf[i + 3]]) as usize;
            let seg_end = i + 2 + seg_len;
            if seg_end > buf.len() {
                break;
            }
            let seg_data = &buf[i + 4..seg_end];
            if seg_data.starts_with(XMP_HEADER) {
                let xmp_bytes = &seg_data[XMP_HEADER.len()..];
                return Ok(Some(String::from_utf8_lossy(xmp_bytes).into_owned()));
            }
            i = seg_end;
        } else if buf[i] == 0xFF && buf[i + 1] != 0x00 {
            // Other marker — skip
            if i + 4 > buf.len() { break; }
            let seg_len = u16::from_be_bytes([buf[i + 2], buf[i + 3]]) as usize;
            i += 2 + seg_len;
        } else {
            i += 1;
        }
    }
    Ok(None)
}

/// Check XMP string for Android Motion Photo markers.
fn xmp_has_motion_photo(xmp: &str) -> bool {
    // Camera:MotionPhoto="1" or GCamera:MotionPhoto="1" or MicroVideo:MicroVideo="1"
    let patterns = [
        r#"Camera:MotionPhoto="1""#,
        r#"Camera:MotionPhoto='1'"#,
        r#"GCamera:MotionPhoto="1""#,
        r#"GCamera:MotionPhoto='1'"#,
        r#"MicroVideo:MicroVideo="1""#,
        r#"MicroVideo:MicroVideo='1'"#,
    ];
    patterns.iter().any(|p| xmp.contains(p))
}

/// Extract a simple attribute value from XMP XML by attribute name.
fn extract_xmp_attr(xmp: &str, attr: &str) -> Option<String> {
    // Look for attr="value" or attr='value'
    let search = format!("{}=\"", attr);
    if let Some(start) = xmp.find(&search) {
        let rest = &xmp[start + search.len()..];
        if let Some(end) = rest.find('"') {
            return Some(rest[..end].to_string());
        }
    }
    let search = format!("{}='", attr);
    if let Some(start) = xmp.find(&search) {
        let rest = &xmp[start + search.len()..];
        if let Some(end) = rest.find('\'') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── xmp_has_motion_photo ──

    #[test]
    fn motion_photo_camera_double_quote() {
        assert!(xmp_has_motion_photo(r#"some xmp Camera:MotionPhoto="1" more"#));
    }

    #[test]
    fn motion_photo_gcamera_double_quote() {
        assert!(xmp_has_motion_photo(r#"some xmp GCamera:MotionPhoto="1" more"#));
    }

    #[test]
    fn motion_photo_microvideo_double_quote() {
        assert!(xmp_has_motion_photo(r#"some xmp MicroVideo:MicroVideo="1" more"#));
    }

    #[test]
    fn motion_photo_single_quote() {
        assert!(xmp_has_motion_photo(r#"Camera:MotionPhoto='1'"#));
    }

    #[test]
    fn motion_photo_empty_string() {
        assert!(!xmp_has_motion_photo(""));
    }

    #[test]
    fn motion_photo_no_match() {
        assert!(!xmp_has_motion_photo(r#"Camera:MotionPhoto="0" some other data"#));
    }

    // ── extract_xmp_attr ──

    #[test]
    fn extract_attr_double_quote() {
        let xmp = r#"apple-fi:Identifier="ABC-123-DEF""#;
        assert_eq!(extract_xmp_attr(xmp, "apple-fi:Identifier"), Some("ABC-123-DEF".to_string()));
    }

    #[test]
    fn extract_attr_single_quote() {
        let xmp = r#"apple-fi:Identifier='XYZ-789'"#;
        assert_eq!(extract_xmp_attr(xmp, "apple-fi:Identifier"), Some("XYZ-789".to_string()));
    }

    #[test]
    fn extract_attr_no_match() {
        let xmp = r#"some other data"#;
        assert_eq!(extract_xmp_attr(xmp, "apple-fi:Identifier"), None);
    }

    #[test]
    fn extract_attr_empty_xmp() {
        assert_eq!(extract_xmp_attr("", "key"), None);
    }

    // ── read_jpeg_xmp ──

    #[test]
    fn read_xmp_nonexistent_file() {
        let result = read_jpeg_xmp(Path::new("/nonexistent/path/file.jpg"));
        assert!(result.is_err());
    }
}
