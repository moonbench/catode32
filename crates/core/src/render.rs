use embedded_graphics::{
    mono_font::{iso_8859_1::{FONT_4X6, FONT_6X10}, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
    text::{Baseline, Text},
};

#[cfg(not(feature = "desktop"))]
use esp_hal::{i2c::master::I2c, Blocking};
#[cfg(not(feature = "desktop"))]
use ssd1306::{
    mode::BufferedGraphicsMode, prelude::*, rotation::DisplayRotation, size::DisplaySize128x64,
    I2CDisplayInterface, Ssd1306,
};

#[cfg(not(feature = "desktop"))]
type DisplayInterface = ssd1306::prelude::I2CInterface<I2c<'static, Blocking>>;
#[cfg(not(feature = "desktop"))]
type Display = Ssd1306<DisplayInterface, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>;

#[cfg(feature = "desktop")]
type Display = desktop_fb::DesktopFramebuffer;

pub struct Sprite {
    pub width: u16,
    pub height: u16,
    pub frames: &'static [&'static [u8]],
    pub fill_frames: Option<&'static [&'static [u8]]>,
}

#[derive(Clone, Copy)]
pub struct SpriteOpts {
    pub frame: usize,
    pub transparent: bool,
    pub invert: bool,
    pub mirror_h: bool,
    pub mirror_v: bool,
    pub transparent_color: bool,
}

impl Default for SpriteOpts {
    fn default() -> Self {
        Self {
            frame: 0,
            transparent: true,
            invert: false,
            mirror_h: false,
            mirror_v: false,
            transparent_color: false,
        }
    }
}

pub struct Renderer {
    display: Display,
    invert_state: bool,
}

impl Renderer {
    #[cfg(not(feature = "desktop"))]
    pub fn new(i2c: I2c<'static, Blocking>) -> Self {
        let interface = I2CDisplayInterface::new(i2c);
        let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();
        display.init().unwrap();
        Self {
            display,
            invert_state: false,
        }
    }

    /// Construct a desktop renderer. The framebuffer lives in-process;
    /// the desktop binary blits it to a `SimulatorDisplay` each frame via
    /// [`Renderer::framebuffer`].
    #[cfg(feature = "desktop")]
    pub fn new() -> Self {
        Self {
            display: desktop_fb::DesktopFramebuffer::new(),
            invert_state: false,
        }
    }

    /// Desktop: borrow the underlying 128x64 1-bpp framebuffer so the
    /// host loop can copy it onto a `SimulatorDisplay`.
    #[cfg(feature = "desktop")]
    pub fn framebuffer(&self) -> &desktop_fb::DesktopFramebuffer {
        &self.display
    }

    /// Cut display panel power (~1-2 mA saving). Drawing still updates the
    /// off-screen buffer; nothing reaches the panel until `power_on()`.
    pub fn power_off(&mut self) {
        self.display.set_display_on(false).ok();
    }

    /// Restore display panel power.
    pub fn power_on(&mut self) {
        self.display.set_display_on(true).ok();
    }

    pub fn set_invert(&mut self, invert: bool) {
        if self.invert_state == invert {
            return;
        }
        self.display.set_invert(invert).ok();
        self.invert_state = invert;
    }

    pub fn clear(&mut self) {
        self.display.clear_buffer();
    }

    pub fn flush(&mut self) {
        self.display.flush().unwrap();
    }

    pub fn draw_text(&mut self, text: &str, pos: Point) {
        let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        Text::with_baseline(text, pos, style, Baseline::Top)
            .draw(&mut self.display)
            .unwrap();
    }

    pub fn draw_text_inverted(&mut self, text: &str, pos: Point) {
        let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::Off);
        Text::with_baseline(text, pos, style, Baseline::Top)
            .draw(&mut self.display)
            .unwrap();
    }

    pub fn draw_text_small(&mut self, text: &str, pos: Point) {
        let style = MonoTextStyle::new(&FONT_4X6, BinaryColor::On);
        Text::with_baseline(text, pos, style, Baseline::Top)
            .draw(&mut self.display)
            .unwrap();
    }

    pub fn draw_text_small_inverted(&mut self, text: &str, pos: Point) {
        let style = MonoTextStyle::new(&FONT_4X6, BinaryColor::Off);
        Text::with_baseline(text, pos, style, Baseline::Top)
            .draw(&mut self.display)
            .unwrap();
    }

    pub fn draw_rect(&mut self, pos: Point, size: Size, filled: bool) {
        let style = if filled {
            PrimitiveStyleBuilder::new()
                .fill_color(BinaryColor::On)
                .build()
        } else {
            PrimitiveStyle::with_stroke(BinaryColor::On, 1)
        };
        Rectangle::new(pos, size)
            .into_styled(style)
            .draw(&mut self.display)
            .unwrap();
    }

    pub fn draw_line(&mut self, start: Point, end: Point) {
        self.draw_line_color(start, end, true);
    }

    /// Draw a line in either ON (`on=true`) or OFF (`on=false`). Used by the
    /// feather render to carve gaps into the otherwise-solid quill outline.
    pub fn draw_line_color(&mut self, start: Point, end: Point, on: bool) {
        let color = if on { BinaryColor::On } else { BinaryColor::Off };
        Line::new(start, end)
            .into_styled(PrimitiveStyle::with_stroke(color, 1))
            .draw(&mut self.display)
            .unwrap();
    }

    /// Tiny filled disc, used by the laser dot and the string tip. Sized for
    /// the small radii (1 or 2) the playing behavior actually requests.
    pub fn draw_circle_filled(&mut self, center: Point, radius: i32) {
        let r2 = radius * radius;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy <= r2 {
                    Pixel(
                        Point::new(center.x + dx, center.y + dy),
                        BinaryColor::On,
                    )
                    .draw(&mut self.display)
                    .ok();
                }
            }
        }
    }

    /// Bresenham midpoint circle outline.
    pub fn draw_circle_outline(&mut self, center: Point, radius: i32) {
        let (cx, cy) = (center.x, center.y);
        let mut x = radius;
        let mut y = 0;
        let mut err = 1 - radius;
        while x >= y {
            self.draw_pixel(Point::new(cx + x, cy + y), true);
            self.draw_pixel(Point::new(cx - x, cy + y), true);
            self.draw_pixel(Point::new(cx + x, cy - y), true);
            self.draw_pixel(Point::new(cx - x, cy - y), true);
            self.draw_pixel(Point::new(cx + y, cy + x), true);
            self.draw_pixel(Point::new(cx - y, cy + x), true);
            self.draw_pixel(Point::new(cx + y, cy - x), true);
            self.draw_pixel(Point::new(cx - y, cy - x), true);
            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }
    }

    pub fn fill_rect_off(&mut self, pos: Point, size: Size) {
        let style = PrimitiveStyleBuilder::new()
            .fill_color(BinaryColor::Off)
            .build();
        Rectangle::new(pos, size)
            .into_styled(style)
            .draw(&mut self.display)
            .unwrap();
    }

    pub fn draw_pixel(&mut self, pos: Point, on: bool) {
        let color = if on { BinaryColor::On } else { BinaryColor::Off };
        Pixel(pos, color).draw(&mut self.display).unwrap();
    }

    pub fn draw_sprite_raw(
        &mut self,
        data: &[u8],
        width: u16,
        height: u16,
        pos: Point,
        opts: SpriteOpts,
    ) {
        let row_bytes = (width as usize + 7) / 8;
        let pixels = (0..height).flat_map(|y| {
            (0..width).filter_map(move |x| {
                let mut sx = x as usize;
                let mut sy = y as usize;
                if opts.mirror_h {
                    sx = width as usize - 1 - sx;
                }
                if opts.mirror_v {
                    sy = height as usize - 1 - sy;
                }
                let byte = data[sy * row_bytes + sx / 8];
                let bit = 7 - (sx % 8);
                let mut on = (byte >> bit) & 1 == 1;
                if opts.invert {
                    on = !on;
                }
                if opts.transparent && on == opts.transparent_color {
                    return None;
                }
                Some(Pixel(
                    Point::new(pos.x + x as i32, pos.y + y as i32),
                    if on { BinaryColor::On } else { BinaryColor::Off },
                ))
            })
        });
        self.display.draw_iter(pixels).ok();
    }

    /// Rotate a 1-bpp packed sprite around its center and draw it.
    ///
    /// `pos` is the top-left of the *unrotated* sprite, matching `draw_sprite_raw`.
    /// The rotated sprite's bounding box is centered on the same point, so the
    /// visual center doesn't drift as the angle changes. Heapless: pixels are
    /// back-mapped from destination to source and streamed straight to the
    /// display via `draw_iter`, so there is no intermediate buffer.
    pub fn draw_sprite_raw_rotated(
        &mut self,
        data: &[u8],
        width: u16,
        height: u16,
        pos: Point,
        angle_deg: f32,
        opts: SpriteOpts,
    ) {
        #[cfg(not(feature = "desktop"))]
        use micromath::F32Ext;
        if angle_deg == 0.0 {
            self.draw_sprite_raw(data, width, height, pos, opts);
            return;
        }
        let w_f = width as f32;
        let h_f = height as f32;
        let cx_src = w_f * 0.5;
        let cy_src = h_f * 0.5;
        let theta = angle_deg * core::f32::consts::PI / 180.0;
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let abs_cos = cos_t.abs();
        let abs_sin = sin_t.abs();
        let new_w = (w_f * abs_cos + h_f * abs_sin).ceil() as i32;
        let new_h = (w_f * abs_sin + h_f * abs_cos).ceil() as i32;
        let cx_dst = new_w as f32 * 0.5;
        let cy_dst = new_h as f32 * 0.5;
        let row_bytes = (width as usize + 7) / 8;
        let origin_x = pos.x - (new_w - width as i32) / 2;
        let origin_y = pos.y - (new_h - height as i32) / 2;
        let w_usize = width as usize;
        let h_usize = height as usize;
        let pixels = (0..new_h).flat_map(move |dy| {
            (0..new_w).filter_map(move |dx| {
                let rx = dx as f32 + 0.5 - cx_dst;
                let ry = dy as f32 + 0.5 - cy_dst;
                let sx_f = cos_t * rx + sin_t * ry + cx_src;
                let sy_f = -sin_t * rx + cos_t * ry + cy_src;
                if sx_f < 0.0 || sy_f < 0.0 {
                    return None;
                }
                let mut sx = sx_f as usize;
                let mut sy = sy_f as usize;
                if sx >= w_usize || sy >= h_usize {
                    return None;
                }
                if opts.mirror_h {
                    sx = w_usize - 1 - sx;
                }
                if opts.mirror_v {
                    sy = h_usize - 1 - sy;
                }
                let byte = data[sy * row_bytes + sx / 8];
                let bit = 7 - (sx % 8);
                let mut on = (byte >> bit) & 1 == 1;
                if opts.invert {
                    on = !on;
                }
                if opts.transparent && on == opts.transparent_color {
                    return None;
                }
                Some(Pixel(
                    Point::new(origin_x + dx, origin_y + dy),
                    if on { BinaryColor::On } else { BinaryColor::Off },
                ))
            })
        });
        self.display.draw_iter(pixels).ok();
    }

    pub fn draw_sprite(&mut self, sprite: &Sprite, pos: Point, opts: SpriteOpts) {
        if let Some(fill_frames) = sprite.fill_frames {
            let fill_idx = opts.frame.min(fill_frames.len().saturating_sub(1));
            let fill_opts = SpriteOpts {
                transparent: true,
                transparent_color: true,
                invert: true,
                ..opts
            };
            self.draw_sprite_raw(
                fill_frames[fill_idx],
                sprite.width,
                sprite.height,
                pos,
                fill_opts,
            );
        }
        let frame_idx = opts.frame.min(sprite.frames.len().saturating_sub(1));
        self.draw_sprite_raw(
            sprite.frames[frame_idx],
            sprite.width,
            sprite.height,
            pos,
            opts,
        );
    }
}

// --- Desktop framebuffer ---------------------------------------------------

/// 128x64 1-bpp framebuffer for desktop builds. Implements
/// `embedded_graphics::DrawTarget<Color = BinaryColor>` plus the
/// SSD1306-shaped helper methods (`clear_buffer`, `flush`, `set_invert`,
/// `set_display_on`) that `Renderer` calls, so the firmware drawing code
/// works unmodified.
#[cfg(feature = "desktop")]
pub mod desktop_fb {
    use embedded_graphics::draw_target::DrawTarget;
    use embedded_graphics::{
        geometry::Size, pixelcolor::BinaryColor, prelude::OriginDimensions, Pixel,
    };

    pub const WIDTH: usize = 128;
    pub const HEIGHT: usize = 64;
    const ROW_BYTES: usize = WIDTH / 8;

    pub struct DesktopFramebuffer {
        /// Row-major 1-bpp packed (MSB first within a byte). Same layout as
        /// the sprites the firmware streams to the SSD1306.
        buf: [u8; WIDTH * HEIGHT / 8],
        invert: bool,
        on: bool,
    }

    impl DesktopFramebuffer {
        pub fn new() -> Self {
            Self {
                buf: [0u8; WIDTH * HEIGHT / 8],
                invert: false,
                on: true,
            }
        }

        pub fn clear_buffer(&mut self) {
            self.buf.fill(0);
        }

        pub fn flush(&mut self) -> Result<(), ()> {
            Ok(())
        }

        pub fn set_invert(&mut self, on: bool) -> Result<(), ()> {
            self.invert = on;
            Ok(())
        }

        pub fn set_display_on(&mut self, on: bool) -> Result<(), ()> {
            self.on = on;
            Ok(())
        }

        pub fn invert(&self) -> bool {
            self.invert
        }

        pub fn is_on(&self) -> bool {
            self.on
        }

        /// True if pixel (x, y) is lit. `invert` is *not* applied here.
        /// The host loop applies it when blitting.
        pub fn pixel(&self, x: usize, y: usize) -> bool {
            if x >= WIDTH || y >= HEIGHT {
                return false;
            }
            let byte = self.buf[y * ROW_BYTES + x / 8];
            (byte >> (7 - (x % 8))) & 1 == 1
        }

        fn set_pixel(&mut self, x: i32, y: i32, on: bool) {
            if x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
                return;
            }
            let (x, y) = (x as usize, y as usize);
            let idx = y * ROW_BYTES + x / 8;
            let mask = 1u8 << (7 - (x % 8));
            if on {
                self.buf[idx] |= mask;
            } else {
                self.buf[idx] &= !mask;
            }
        }
    }

    impl Default for DesktopFramebuffer {
        fn default() -> Self {
            Self::new()
        }
    }

    impl OriginDimensions for DesktopFramebuffer {
        fn size(&self) -> Size {
            Size::new(WIDTH as u32, HEIGHT as u32)
        }
    }

    impl DrawTarget for DesktopFramebuffer {
        type Color = BinaryColor;
        type Error = core::convert::Infallible;

        fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = Pixel<Self::Color>>,
        {
            for Pixel(p, c) in pixels {
                self.set_pixel(p.x, p.y, c.is_on());
            }
            Ok(())
        }
    }
}
