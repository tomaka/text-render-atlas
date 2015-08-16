use std::io::Read;

use LoadError;
use TextureData;

mod freetype;

/// Structure containing informations about a character of a font.
#[derive(Copy, Clone, Debug)]
pub struct CharacterInfos {
    /// Coordinates of the character top-left hand corner on the font's texture.
    pub tex_coords: (f32, f32),

    /// Width and height of character in texture units.
    pub tex_size: (f32, f32),

    /// Size of the character in EMs.
    pub size: (f32, f32),

    /// Number of EMs between the bottom of the character and the base line of text.
    pub height_over_line: f32,

    /// Number of EMs at the left of the character.
    pub left_padding: f32,

    /// Number of EMs at the right of the character.
    pub right_padding: f32,
}

/// Loads a font.
#[inline]
pub fn load_font<R>(data: R, font_size: u32)
                    -> Result<(TextureData, Vec<(char, CharacterInfos)>), LoadError>
                    where R: Read
{
    freetype::load_font(data, font_size)
}
