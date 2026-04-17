use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{DrawTarget, RgbColor},
};

/// Framebuffer compatibile con embedded-graphics che usa un buffer RGB888
pub struct ImageFramebuffer {
    pub width: u32,
    pub height: u32,
    pub buffer: Vec<u8>, // RGB interleaved (Rgb888)
}

impl DrawTarget for ImageFramebuffer {
    type Color = Rgb565;
    type Error = ();

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics_core::Pixel<Self::Color>>,
    {
        for pixel in pixels {
            let x = pixel.0.x as u32;
            let y = pixel.0.y as u32;

            if x < self.width && y < self.height {
                let idx = ((y * self.width + x) * 3) as usize;
                if idx + 2 < self.buffer.len() {
                    let color = pixel.1;
                    // Converti Rgb565 a Rgb888
                    let r = (color.r() as u16 * 255 / 31) as u8;
                    let g = (color.g() as u16 * 255 / 63) as u8;
                    let b = (color.b() as u16 * 255 / 31) as u8;
                    self.buffer[idx] = r;
                    self.buffer[idx + 1] = g;
                    self.buffer[idx + 2] = b;
                }
            }
        }
        Ok(())
    }
}

impl embedded_graphics_core::geometry::OriginDimensions for ImageFramebuffer {
    fn size(&self) -> embedded_graphics_core::geometry::Size {
        embedded_graphics_core::geometry::Size::new(self.width, self.height)
    }
}
