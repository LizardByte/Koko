//! Validate downloaded artwork before publishing it in the cache.
pub(super) fn validate(bytes: &[u8]) -> Result<(), String> {
    if let Ok(format) = image::guess_format(bytes) {
        return image::ImageReader::with_format(std::io::Cursor::new(bytes), format)
            .decode()
            .map(|_| ())
            .map_err(|error| format!("Invalid artwork image: {error}"));
    }
    let text =
        std::str::from_utf8(bytes).map_err(|_| "Artwork is not a supported image".to_string())?;
    let document = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..roxmltree::ParsingOptions::default()
        },
    )
    .map_err(|error| format!("Invalid SVG artwork: {error}"))?;
    let root = document.root_element().tag_name();
    if root.name() == "svg" && root.namespace() == Some("http://www.w3.org/2000/svg") {
        Ok(())
    } else {
        Err("Artwork is not an SVG document".into())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn image_validation_accepts_raster_and_svg_and_rejects_error_pages() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbImage::new(1, 1)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        assert!(super::validate(png.get_ref()).is_ok());
        assert!(
            super::validate(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"/>"#)
                .is_ok()
        );
        assert!(super::validate(b"<html><body>503 unavailable</body></html>").is_err());
        assert!(super::validate(b"not image").is_err());
        assert!(super::validate(&png.get_ref()[..16]).is_err());
    }
}
