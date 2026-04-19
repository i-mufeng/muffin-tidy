#[cfg(test)]
mod tests {
    use std::path::Path;
    use crate::media::{MediaType, format::detect_media_type};

    fn img(ext: &str) -> bool {
        matches!(detect_media_type(Path::new(&format!("file.{}", ext))), Some(MediaType::Img))
    }

    fn vdo(ext: &str) -> bool {
        matches!(detect_media_type(Path::new(&format!("file.{}", ext))), Some(MediaType::Vdo))
    }

    fn none(ext: &str) -> bool {
        detect_media_type(Path::new(&format!("file.{}", ext))).is_none()
    }

    #[test]
    fn image_extensions() {
        assert!(img("jpg"));
        assert!(img("jpeg"));
        assert!(img("JPG"));   // case-insensitive
        assert!(img("HEIC"));
        assert!(img("png"));
        assert!(img("dng"));
        assert!(img("cr2"));
        assert!(img("arw"));
    }

    #[test]
    fn video_extensions() {
        assert!(vdo("mp4"));
        assert!(vdo("MP4"));
        assert!(vdo("mov"));
        assert!(vdo("MOV"));
        assert!(vdo("mkv"));
        assert!(vdo("3gp"));
        assert!(vdo("mts"));
    }

    #[test]
    fn unsupported_extensions() {
        assert!(none("txt"));
        assert!(none("pdf"));
        assert!(none("exe"));
        assert!(none(""));
    }

    #[test]
    fn no_extension() {
        assert!(detect_media_type(Path::new("noextension")).is_none());
    }
}
