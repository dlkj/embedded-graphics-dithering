use core::iter::{Map, Repeat, Zip};
use embedded_graphics::pixelcolor::{BinaryColor, Gray8};
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use sh1106::interface::DisplayInterface;
use sh1106::mode::GraphicsMode;

const NOISE: [&[u8; 256]; 16] = [
    include_bytes!("../../bluenoise/resources/16x16_raw/LDR_LLL1_0.gray"),
    include_bytes!("../../bluenoise/resources/16x16_raw/LDR_LLL1_1.gray"),
    include_bytes!("../../bluenoise/resources/16x16_raw/LDR_LLL1_2.gray"),
    include_bytes!("../../bluenoise/resources/16x16_raw/LDR_LLL1_3.gray"),
    include_bytes!("../../bluenoise/resources/16x16_raw/LDR_LLL1_4.gray"),
    include_bytes!("../../bluenoise/resources/16x16_raw/LDR_LLL1_5.gray"),
    include_bytes!("../../bluenoise/resources/16x16_raw/LDR_LLL1_6.gray"),
    include_bytes!("../../bluenoise/resources/16x16_raw/LDR_LLL1_7.gray"),
    include_bytes!("../../bluenoise/resources/16x16_raw/LDR_LLL1_8.gray"),
    include_bytes!("../../bluenoise/resources/16x16_raw/LDR_LLL1_9.gray"),
    include_bytes!("../../bluenoise/resources/16x16_raw/LDR_LLL1_10.gray"),
    include_bytes!("../../bluenoise/resources/16x16_raw/LDR_LLL1_11.gray"),
    include_bytes!("../../bluenoise/resources/16x16_raw/LDR_LLL1_12.gray"),
    include_bytes!("../../bluenoise/resources/16x16_raw/LDR_LLL1_13.gray"),
    include_bytes!("../../bluenoise/resources/16x16_raw/LDR_LLL1_14.gray"),
    include_bytes!("../../bluenoise/resources/16x16_raw/LDR_LLL1_15.gray"),
];

pub struct DitheringAdaptor<D, E>
where
    D: DrawTarget<Color = BinaryColor, Error = E>,
{
    pub display: D,
    pub frame: usize,
}

impl<D, E> Dimensions for DitheringAdaptor<D, E>
where
    D: DrawTarget<Color = BinaryColor, Error = E>,
{
    fn bounding_box(&self) -> Rectangle {
        self.display.bounding_box()
    }
}

impl<D, E> DrawTarget for DitheringAdaptor<D, E>
where
    D: DrawTarget<Color = BinaryColor, Error = E>,
{
    type Color = Gray8;
    type Error = E;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.display.draw_iter(DitheringIntoIter {
            inner: pixels,
            frame: self.frame,
        })
    }
}

struct DitheringIntoIter<I> {
    inner: I,
    frame: usize,
}

impl<I> IntoIterator for DitheringIntoIter<I>
where
    I: IntoIterator<Item = Pixel<Gray8>>,
{
    type Item = Pixel<BinaryColor>;
    type IntoIter =
        Map<Zip<I::IntoIter, Repeat<&'static [u8; 256]>>, fn((I::Item, &[u8; 256])) -> Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        let noise_frame = NOISE[self.frame];

        self.inner
            .into_iter()
            .zip(core::iter::repeat(noise_frame))
            .map(|(Pixel(p, c), n)| {
                let noise_p = n[usize::try_from(p.x % 16 + 16 * (p.y % 16)).unwrap()];
                Pixel(
                    p,
                    if c.luma() == 0 || c.luma() < noise_p {
                        BinaryColor::Off
                    } else {
                        BinaryColor::On
                    },
                )
            })
    }
}

impl<DI> DitheringAdaptor<GraphicsMode<DI>, core::convert::Infallible>
where
    DI: DisplayInterface,
{
    pub fn clear(&mut self) {
        self.display.clear()
    }
    pub fn flush(&mut self) -> Result<(), DI::Error> {
        self.frame = (self.frame + 1) % NOISE.len();
        self.display.flush()
    }
}
