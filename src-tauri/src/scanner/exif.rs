use std::path::Path;

#[derive(Debug, Default)]
pub struct ExifData {
    pub taken_at: Option<i64>,
    pub taken_at_source: String,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub width_px: Option<i64>,
    pub height_px: Option<i64>,
}

pub fn extract(path: &Path) -> ExifData {
    let mut data = ExifData {
        taken_at_source: "none".into(),
        ..Default::default()
    };

    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return fallback_mtime(path, data),
    };

    let mut bufreader = std::io::BufReader::new(file);
    let exifreader = exif::Reader::new();

    let exif = match exifreader.read_from_container(&mut bufreader) {
        Ok(e) => e,
        Err(_) => return fallback_mtime(path, data),
    };

    // DateTimeOriginal
    if let Some(field) = exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY) {
        if let exif::Value::Ascii(ref vec) = field.value {
            if let Some(bytes) = vec.first() {
                if let Ok(s) = std::str::from_utf8(bytes) {
                    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S") {
                        data.taken_at = Some(dt.and_utc().timestamp());
                        data.taken_at_source = "exif".into();
                    }
                }
            }
        }
    }

    // Camera Make
    if let Some(field) = exif.get_field(exif::Tag::Make, exif::In::PRIMARY) {
        data.camera_make = Some(field.display_value().to_string().trim().to_string());
    }

    // Camera Model
    if let Some(field) = exif.get_field(exif::Tag::Model, exif::In::PRIMARY) {
        data.camera_model = Some(field.display_value().to_string().trim().to_string());
    }

    // Dimensions from EXIF (may not be present)
    if let Some(field) = exif.get_field(exif::Tag::ImageWidth, exif::In::PRIMARY) {
        if let exif::Value::Long(ref v) = field.value {
            data.width_px = v.first().map(|&x| x as i64);
        } else if let exif::Value::Short(ref v) = field.value {
            data.width_px = v.first().map(|&x| x as i64);
        }
    }

    if let Some(field) = exif.get_field(exif::Tag::ImageLength, exif::In::PRIMARY) {
        if let exif::Value::Long(ref v) = field.value {
            data.height_px = v.first().map(|&x| x as i64);
        } else if let exif::Value::Short(ref v) = field.value {
            data.height_px = v.first().map(|&x| x as i64);
        }
    }

    if data.taken_at.is_none() {
        return fallback_mtime(path, data);
    }

    data
}

fn fallback_mtime(path: &Path, mut data: ExifData) -> ExifData {
    if let Ok(meta) = std::fs::metadata(path) {
        if let Ok(modified) = meta.modified() {
            let epoch = modified
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            data.taken_at = Some(epoch);
            data.taken_at_source = "file_mtime".into();
        }
    }
    data
}
