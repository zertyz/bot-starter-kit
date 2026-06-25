use super::super::config::TypographyTheme;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TextRotation {
    None,
    Rotate270,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TextBoxPx {
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct TextKey {
    text: String,
    font_px: u32,
    rotation: TextRotation,
}

#[derive(Default, Debug)]
pub struct FontMetricsCache {
    cache: HashMap<TextKey, TextBoxPx>,
}

impl FontMetricsCache {
    pub fn measure(&mut self, text: &str, font_px: u32, rotation: TextRotation, typography: &TypographyTheme) -> TextBoxPx {
        let key = TextKey { text: text.to_string(), font_px, rotation };

        if let Some(v) = self
            .cache
            .get(&key)
        {
            return *v;
        }

        let raw_width = (text
            .chars()
            .count() as f64
            * font_px as f64
            * typography.avg_advance_em)
            .ceil() as i32;
        let raw_height = (font_px as f64 * typography.line_height_em).ceil() as i32;

        let value = match rotation {
            TextRotation::None => TextBoxPx { width: raw_width, height: raw_height },
            TextRotation::Rotate270 => TextBoxPx { width: raw_height, height: raw_width },
        };

        self.cache
            .insert(key, value);
        value
    }

    pub fn len(&self) -> usize {
        self.cache
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache
            .is_empty()
    }
}
