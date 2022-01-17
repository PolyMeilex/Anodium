use std::io::Read;

use smithay::{
    backend::renderer::{
        gles2::{Gles2Error, Gles2Frame, Gles2Renderer, Gles2Texture},
        Frame, Texture,
    },
    desktop::space::{RenderElement, SpaceOutputTuple},
    utils::{Logical, Point, Rectangle, Size, Transform},
};
use xcursor::{
    parser::{parse_xcursor, Image},
    CursorTheme,
};

static FALLBACK_CURSOR_DATA: &[u8] = include_bytes!("../../resources/cursor.rgba");

pub struct Cursor {
    icons: Vec<Image>,
    size: u32,
    start_time: std::time::Instant,
}

impl Cursor {
    pub fn load(log: &::slog::Logger) -> Cursor {
        let name = std::env::var("XCURSOR_THEME")
            .ok()
            .unwrap_or_else(|| "default".into());
        let size = std::env::var("XCURSOR_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(24);

        let theme = CursorTheme::load(&name);
        let icons = load_icon(&theme)
            .map_err(|err| {
                slog::warn!(
                    log,
                    "Unable to load xcursor: {}, using fallback cursor",
                    err
                )
            })
            .unwrap_or_else(|_| {
                vec![Image {
                    size: 32,
                    width: 64,
                    height: 64,
                    xhot: 1,
                    yhot: 1,
                    delay: 1,
                    pixels_rgba: Vec::from(FALLBACK_CURSOR_DATA),
                    pixels_argb: vec![], //unused
                }]
            });

        Cursor {
            icons,
            size,
            start_time: std::time::Instant::now(),
        }
    }

    pub fn get_image(&self, scale: u32) -> Image {
        let size = self.size * scale;
        let millis = self.start_time.elapsed().as_millis();

        frame(millis, size, &self.icons)
    }
}

fn nearest_images(size: u32, images: &[Image]) -> impl Iterator<Item = &Image> {
    // Follow the nominal size of the cursor to choose the nearest
    let nearest_image = images
        .iter()
        .min_by_key(|image| (size as i32 - image.size as i32).abs())
        .unwrap();

    images.iter().filter(move |image| {
        image.width == nearest_image.width && image.height == nearest_image.height
    })
}

fn frame(mut millis: u128, size: u32, images: &[Image]) -> Image {
    let total = nearest_images(size, images).fold(0, |acc, image| acc + image.delay) as u128;
    millis %= total;

    for img in nearest_images(size, images) {
        if millis < img.delay as u128 {
            return img.clone();
        }
        millis -= img.delay as u128;
    }

    unreachable!()
}

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("Theme has no default cursor")]
    NoDefaultCursor,
    #[error("Error opening xcursor file: {0}")]
    File(#[from] std::io::Error),
    #[error("Failed to parse XCursor file")]
    Parse,
}

fn load_icon(theme: &CursorTheme) -> Result<Vec<Image>, Error> {
    let icon_path = theme.load_icon("default").ok_or(Error::NoDefaultCursor)?;
    let mut cursor_file = std::fs::File::open(&icon_path)?;
    let mut cursor_data = Vec::new();
    cursor_file.read_to_end(&mut cursor_data)?;
    parse_xcursor(&cursor_data).ok_or(Error::Parse)
}

#[derive(Clone, Debug)]
pub struct PointerElement {
    texture: Gles2Texture,
    position: Point<i32, Logical>,
    size: Size<i32, Logical>,
}

impl PointerElement {
    pub fn new(texture: Gles2Texture, relative_pointer_pos: Point<i32, Logical>) -> PointerElement {
        let size = texture.size().to_logical(1, Transform::Normal);
        PointerElement {
            texture,
            position: relative_pointer_pos,
            size,
        }
    }
}

impl RenderElement<Gles2Renderer, Gles2Frame, Gles2Error, Gles2Texture> for PointerElement {
    fn id(&self) -> usize {
        0
    }

    fn geometry(&self) -> Rectangle<i32, Logical> {
        Rectangle::from_loc_and_size(self.position, self.size)
    }

    fn accumulated_damage(
        &self,
        _: Option<SpaceOutputTuple<'_, '_>>,
    ) -> Vec<Rectangle<i32, Logical>> {
        vec![Rectangle::from_loc_and_size((0, 0), self.size)]
    }

    fn draw(
        &self,
        _renderer: &mut Gles2Renderer,
        frame: &mut Gles2Frame,
        scale: f64,
        damage: &[Rectangle<i32, Logical>],
        _log: &slog::Logger,
    ) -> Result<(), Gles2Error> {
        frame.render_texture_at(
            &self.texture,
            self.position
                .to_f64()
                .to_physical(scale as f64)
                .to_i32_round(),
            1,
            scale as f64,
            Transform::Normal,
            &*damage
                .iter()
                .map(|rect| rect.to_buffer(1, Transform::Normal, &self.size))
                .collect::<Vec<_>>(),
            1.0,
        )?;
        Ok(())
    }
}
