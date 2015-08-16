extern crate freetype_sys as freetype;
extern crate libc;

use TextureData;
use LoadError;

use backend::CharacterInfos;

use std::io::Read;
use std::{iter, ptr, mem, slice, cmp};

/// Creates a new texture representing a font stored in a `FontTexture`.
pub fn load_font<R>(font: R, font_size: u32)
                    -> Result<(TextureData, Vec<(char, CharacterInfos)>), LoadError> where R: Read
{
    // building the freetype library
    // FIXME: call FT_Done_Library
    let library = unsafe {
        // taken from https://github.com/PistonDevelopers/freetype-rs/blob/master/src/library.rs
        extern "C" fn alloc_library(_memory: freetype::FT_Memory, size: libc::c_long)
                                    -> *mut libc::c_void
        {
            unsafe {
                libc::malloc(size as libc::size_t)
            }
        }
        extern "C" fn free_library(_memory: freetype::FT_Memory, block: *mut libc::c_void) {
            unsafe {
                libc::free(block)
            }
        }
        extern "C" fn realloc_library(_memory: freetype::FT_Memory,
                                      _cur_size: libc::c_long,
                                      new_size: libc::c_long,
                                      block: *mut libc::c_void) -> *mut libc::c_void {
            unsafe {
                libc::realloc(block, new_size as libc::size_t)
            }
        }
        static mut MEMORY: freetype::FT_MemoryRec = freetype::FT_MemoryRec {
            user: 0 as *mut libc::c_void,
            alloc: alloc_library,
            free: free_library,
            realloc: realloc_library,
        };

        let mut raw = ptr::null_mut();
        if freetype::FT_New_Library(&mut MEMORY, &mut raw) != freetype::FT_Err_Ok {
            return Err(LoadError::WrongFormat);
        }
        freetype::FT_Add_Default_Modules(raw);

        raw
    };

    // building the freetype face object
    let font: Vec<u8> = font.bytes().map(|c| c.unwrap()).collect();

    let face: freetype::FT_Face = unsafe {
        let mut face = ptr::null_mut();
        let err = freetype::FT_New_Memory_Face(library, font.as_ptr(),
                                               font.len() as freetype::FT_Long, 0, &mut face);
        if err == freetype::FT_Err_Ok {
            face
        } else {
            return Err(LoadError::WrongFormat);
        }
    };

    // computing the list of characters in the font
    let characters_list = unsafe {
        // TODO: unresolved symbol
        /*if freetype::FT_Select_CharMap(face, freetype::FT_ENCODING_UNICODE) != 0 {
            return Err(());
        }*/

        let mut result = Vec::new();

        let mut g: freetype::FT_UInt = mem::uninitialized();
        let mut c = freetype::FT_Get_First_Char(face, &mut g);

        while g != 0 {
            result.push(mem::transmute(c as u32));     // TODO: better solution?
            c = freetype::FT_Get_Next_Char(face, c, &mut g);
        }

        result
    };

    // building the infos
    let result = unsafe {
        build_font_image(face, characters_list, font_size)
    };

    // TODO: cleanup the font

    Ok(result)
}

unsafe fn build_font_image(face: freetype::FT_Face, characters_list: Vec<char>, font_size: u32)
                           -> (TextureData, Vec<(char, CharacterInfos)>)
{
    // a margin around each character to prevent artifacts
    const MARGIN: u32 = 2;

    // setting the right pixel size
    if freetype::FT_Set_Pixel_Sizes(face, font_size, font_size) != 0 {
        panic!();
    }

    // this variable will store the texture data
    // we set an arbitrary capacity that we think will match what we will need
    let mut texture_data: Vec<f32> = Vec::with_capacity(characters_list.len() *
                                                        font_size as usize * font_size as usize);

    // the width is chosen more or less arbitrarily, because we can store everything as long as
    //  the texture is at least as wide as the widest character
    // we just try to estimate a width so that width ~= height
    let texture_width = get_nearest_po2(cmp::max(font_size * 2 as u32,
        ((((characters_list.len() as u32) * font_size * font_size) as f32).sqrt()) as u32));

    // we store the position of the "cursor" in the destination texture
    // this cursor points to the top-left pixel of the next character to write on the texture
    let mut cursor_offset = (0u32, 0u32);

    // number of rows to skip at next carriage return
    let mut rows_to_skip = 0u32;

    // now looping through the list of characters, filling the texture and returning the informations
    let mut em_pixels = font_size as f32;
    let mut characters_infos: Vec<(char, CharacterInfos)> = characters_list.into_iter().filter_map(|character| {
        // loading wanted glyph in the font face
        if freetype::FT_Load_Glyph(face, freetype::FT_Get_Char_Index(face, character as freetype::FT_ULong), freetype::FT_LOAD_RENDER) != 0 {
            return None;
        }
        let bitmap = &(*(*face).glyph).bitmap;

        // adding a left margin before our character to prevent artifacts
        cursor_offset.0 += MARGIN;

        // computing em_pixels
        // FIXME: this is hacky
        if character == 'M' {
            em_pixels = bitmap.rows as f32;
        }

        // carriage return our cursor if we don't have enough room to write the next caracter
        // we add a margin to prevent artifacts
        if cursor_offset.0 + (bitmap.width as u32) + MARGIN >= texture_width {
            assert!(bitmap.width as u32 <= texture_width);       // if this fails, we should increase texture_width
            cursor_offset.0 = 0;
            cursor_offset.1 += rows_to_skip;
            rows_to_skip = 0;
        }

        // if the texture data buffer has not enough lines, adding some
        if rows_to_skip < MARGIN + bitmap.rows as u32 {
            let diff = MARGIN + (bitmap.rows as u32) - rows_to_skip;
            rows_to_skip = MARGIN + bitmap.rows as u32;
            texture_data.extend(iter::repeat(0.0).take((diff * texture_width) as usize));
        }

        // copying the data to the texture
        let offset_x_before_copy = cursor_offset.0;
        if bitmap.rows >= 1 {
            let destination = &mut texture_data[(cursor_offset.0 + cursor_offset.1 * texture_width) as usize ..];
            let source = mem::transmute(bitmap.buffer);
            let source = slice::from_raw_parts(source, destination.len());

            for y in (0 .. bitmap.rows as u32) {
                let source = &source[(y * bitmap.width as u32) as usize ..];
                let destination = &mut destination[(y * texture_width) as usize ..];

                for x in (0 .. bitmap.width) {
                    // the values in source are bytes between 0 and 255, but we want floats between 0 and 1
                    let val: u8 = *source.get(x as usize).unwrap();
                    let val = (val as f32) / 255.0;
                    let dest = destination.get_mut(x as usize).unwrap();
                    *dest = val;
                }
            }

            cursor_offset.0 += bitmap.width as u32;
            debug_assert!(cursor_offset.0 <= texture_width);
        }

        // filling infos about that character
        // tex_size and tex_coords are in pixels for the moment ; they will be divided
        // by the texture dimensions later
        let left_padding = (*(*face).glyph).bitmap_left;

        Some((character, CharacterInfos {
            tex_size: (bitmap.width as f32, bitmap.rows as f32),
            tex_coords: (offset_x_before_copy as f32, cursor_offset.1 as f32),
            size: (bitmap.width as f32, bitmap.rows as f32),
            left_padding: left_padding as f32,
            right_padding: ((*(*face).glyph).advance.x as i32 - bitmap.width * 64 - left_padding * 64) as f32 / 64.0,
            height_over_line: (*(*face).glyph).bitmap_top as f32,
        }))
    }).collect();

    // adding blank lines at the end until the height of the texture is a power of two
    {
        let current_height = texture_data.len() as u32 / texture_width;
        let requested_height = get_nearest_po2(current_height);
        texture_data.extend(iter::repeat(0.0).take((texture_width * (requested_height - current_height)) as usize));
    }

    // now our texture is finished
    // we know its final dimensions, so we can divide all the pixels values into (0,1) range
    assert!((texture_data.len() as u32 % texture_width) == 0);
    let texture_height = (texture_data.len() as u32 / texture_width) as f32;
    let float_texture_width = texture_width as f32;
    for chr in characters_infos.iter_mut() {
        chr.1.tex_size.0 /= float_texture_width;
        chr.1.tex_size.1 /= texture_height;
        chr.1.tex_coords.0 /= float_texture_width;
        chr.1.tex_coords.1 /= texture_height;
        chr.1.size.0 /= em_pixels;
        chr.1.size.1 /= em_pixels;
        chr.1.left_padding /= em_pixels;
        chr.1.right_padding /= em_pixels;
        chr.1.height_over_line /= em_pixels;
    }

    // returning
    (TextureData {
        data: texture_data,
        width: texture_width,
        height: texture_height as u32,
    }, characters_infos)
}

/// Function that will calculate the nearest power of two.
fn get_nearest_po2(mut x: u32) -> u32 {
    assert!(x > 0);
    x -= 1;
    x = x | (x >> 1);
    x = x | (x >> 2);
    x = x | (x >> 4);
    x = x | (x >> 8);
    x = x | (x >> 16);
    x + 1
}
