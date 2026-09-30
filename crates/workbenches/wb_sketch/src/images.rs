//! Reference images: pictures on the sketch plane to draw over. Their
//! bytes are document assets; decoded once each, they are laid over the
//! viewport while their sketch is edited.

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use crate::sketch::{ReferenceImage, Vec2D};

/// The longest side a picture is shown at, in pixels: plenty to trace, and
/// a texture every GPU takes.
const LONGEST_PX: u32 = 2048;

/// A decoded picture, RGBA rows from the top.
#[derive(Debug)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<Vec<u8>>,
}

/// Decode a PNG or JPEG file's bytes, no larger than [`LONGEST_PX`].
pub fn decode(bytes: &[u8]) -> Result<Decoded, String> {
    let picture = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let picture = if picture.width().max(picture.height()) > LONGEST_PX {
        picture.resize(
            LONGEST_PX,
            LONGEST_PX,
            image::imageops::FilterType::Triangle,
        )
    } else {
        picture
    };
    let rgba = picture.to_rgba8();
    Ok(Decoded {
        width: rgba.width(),
        height: rgba.height(),
        rgba: Arc::new(rgba.into_raw()),
    })
}

/// Pictures decoded so far, by asset: `None` for one that would not decode.
#[derive(Default)]
pub struct Cache(std::cell::RefCell<HashMap<Uuid, Option<Arc<Decoded>>>>);

impl Cache {
    /// The asset's picture, decoded the first time it is asked for.
    pub fn get(
        &self,
        asset: Uuid,
        bytes: impl FnOnce() -> Option<Vec<u8>>,
    ) -> Option<Arc<Decoded>> {
        self.0
            .borrow_mut()
            .entry(asset)
            .or_insert_with(|| bytes().and_then(|b| decode(&b).ok()).map(Arc::new))
            .clone()
    }
}

/// The picture's corners in sketch coordinates: top left, top right,
/// bottom right, bottom left, turned about its middle.
pub fn corners(image: &ReferenceImage, aspect: f32) -> [Vec2D; 4] {
    let (w, h) = (image.width / 2.0, image.width * aspect / 2.0);
    let (sin, cos) = image.angle_deg.to_radians().sin_cos();
    let at = |x: f32, y: f32| {
        Vec2D::new(
            image.center.x + x * cos - y * sin,
            image.center.y + x * sin + y * cos,
        )
    };
    [at(-w, h), at(w, h), at(w, -h), at(-w, -h)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture keeps its proportions and turns about its middle.
    #[test]
    fn a_picture_lies_where_it_is_placed() {
        let image = ReferenceImage {
            id: Uuid::nil(),
            asset: Uuid::nil(),
            center: Vec2D::new(10.0, 5.0),
            width: 40.0,
            angle_deg: 90.0,
            opacity: 0.5,
        };
        // Twice as wide as tall: 40 × 20, turned a quarter.
        let [top_left, _, bottom_right, _] = corners(&image, 0.5);
        assert!(
            (top_left.x - 0.0).abs() < 1e-4 && (top_left.y - -15.0).abs() < 1e-4,
            "{top_left:?}"
        );
        assert!((bottom_right.x - 20.0).abs() < 1e-4 && (bottom_right.y - 25.0).abs() < 1e-4);
    }

    /// A PNG decodes to its size and pixels.
    #[test]
    fn a_png_decodes() {
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let decoded = decode(&png).unwrap();
        assert_eq!((decoded.width, decoded.height), (3, 2));
        assert_eq!(&decoded.rgba[..4], &[10, 20, 30, 255]);
        assert!(decode(b"not a picture").is_err());
    }
}
