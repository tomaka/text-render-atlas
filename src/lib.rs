use std::io::Read;

mod backend;

/// Error while loading the font.
#[derive(Debug, Copy, Clone)]
pub enum LoadError {
    WrongFormat,
}

/// The data of the texture atlas.
pub struct TextureData {
    /// Each element should be a texel.
    ///
    /// `data.len()` must be equal to `width * height`.
    ///
    /// **Important**: the data is bottom-to-top, because that's what OpenGL requests. The first
    /// line is the bottom line of texels.
    pub data: Vec<f32>,

    /// Width of the texture in number of texels.
    pub width: u32,

    /// Height of the texture in number of texels.
    pub height: u32,
}

/// Information about a single sprite that you must draw.
pub struct SpriteInfos {
    pub left_position: f32,
    pub top_position: f32,
    pub right_position: f32,
    pub bottom_position: f32,

    pub left_tex_coords: f32,
    pub top_tex_coords: f32,
    pub right_tex_coords: f32,
    pub bottom_tex_coords: f32,
}

pub struct Font {
    character_infos: Vec<(char, backend::CharacterInfos)>,
}

impl Font {
    /// Loads a font from a font file data.
    ///
    /// The `font_size` parameter is here to indicate the size of the texture atlas, which means
    /// that a greater size will give you a better quality.
    ///
    /// You can draw any text size with any font size, but drawing a large text with a small font
    /// size will be ugly, and using a large font size will eat up more memory.
    pub fn load<R>(data: R, font_size: u32) -> Result<(Font, TextureData), LoadError> 
                   where R: Read
    {
        let (tex, infos) = try!(backend::load_font(data, font_size));
        Ok((Font { character_infos: infos }, tex))
    }

    /// Calculates the position of each character of a text.
    ///
    /// This function assumes that:
    ///  - The bottom-left hand corner of the text is at position `(0.0, 0.0)`.
    ///  - The height of an EM is `1.0`.
    ///
    /// If you want to move the text, just add a value to each returned coordinate. If you want to
    /// resize the text, just multiply the value of each returned coordinate.
    ///
    /// The function returns the list of sprites plus the total width of the text. For example if
    /// you want to center-align the text, just move it by `width / 2.0` units to the left.
    pub fn calculate(&self, text: &str) -> (Vec<SpriteInfos>, f32) {
        let mut output = Vec::new();
        let mut total_text_width = 0.0;

        // iterating over the characters of the string
        for character in text.chars() {     // FIXME: wrong, but only thing stable
            let infos = if let Some(infos) = self.character_infos
                                                 .iter().find(|&&(chr, _)| chr == character)
            {
                &infos.1
            } else {
                continue;
            };

            total_text_width += infos.left_padding;

            // calculating coords
            output.push(SpriteInfos {
                left_position: total_text_width,
                top_position: infos.height_over_line,
                right_position: total_text_width + infos.size.0,
                bottom_position: infos.height_over_line - infos.size.1,

                left_tex_coords: infos.tex_coords.0,
                top_tex_coords: infos.tex_coords.1,
                right_tex_coords: infos.tex_coords.0 + infos.tex_size.0,
                bottom_tex_coords: infos.tex_coords.1 + infos.tex_size.1,
            });

            // going to next char
            total_text_width += infos.size.0 + infos.right_padding;
        }

        (output, total_text_width)
    }
}
