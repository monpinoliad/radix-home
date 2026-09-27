// Based on https://github.com/igiona/rs-watch/blob/main/src/ui_task/display_line_buffer_provider.rs

use embedded_graphics_core::{
    draw_target::DrawTarget,
    geometry::{Point, Size},
    pixelcolor::{Rgb565, raw::RawU16},
    primitives::Rectangle,
};
use slint::platform::software_renderer::{LineBufferProvider, Rgb565Pixel};

/// Lets Slint render one scanline at a time straight into any embedded-graphics display,
/// so we never need a full 240x240 framebuffer in RAM.
pub struct DrawBuffer<'a, T> {
    pub display: &'a mut T,
    pub line_buffer: &'a mut [Rgb565Pixel],
}

impl<T: DrawTarget<Color = Rgb565>> LineBufferProvider for DrawBuffer<'_, T> {
    type TargetPixel = Rgb565Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ) {
        let pixels = &mut self.line_buffer[range.clone()];
        render_fn(pixels);

        self.display
            .fill_contiguous(
                &Rectangle::new(
                    Point::new(range.start as _, line as _),
                    Size::new(range.len() as _, 1),
                ),
                pixels.iter().map(|p| RawU16::new(p.0).into()),
            )
            .map_err(drop)
            .unwrap();
    }
}
