use std::{error::Error, fmt};

pub const MAX_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_PLANES: usize = 32;
pub const MAX_EDGE: u32 = 16384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgba,
    Bgra,
    Bgrx,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    None,
    Coverage,
    Premultiplied,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    R0,
    R90,
    R180,
    R270,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Overlay,
    Cursor,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Destination {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct Plane<'a> {
    pub id: u32,
    pub z: i32,
    pub kind: Kind,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixels: &'a [u8],
    pub format: PixelFormat,
    pub blend: Blend,
    pub opacity: u16,
    pub crop: Crop,
    pub destination: Destination,
    /// Reflect in source coordinates before the clockwise quarter turn.
    pub rotation: Rotation,
    pub reflect_x: bool,
    pub reflect_y: bool,
}

#[derive(Debug)]
pub struct Scanout<'a> {
    pub width: u32,
    pub height: u32,
    pub planes: Vec<Plane<'a>>,
}
#[derive(Debug)]
pub struct ComposedFrame {
    pub width: u32,
    pub height: u32,
    pub pixels_rgba: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionError {
    InvalidDimensions,
    BufferLayout,
    SourceBounds,
    UnsupportedScaling,
    PlaneOrder,
    PrimaryCoverage,
    TooManyPlanes,
    PixelBudget,
    InvalidPremultipliedPixel,
}
impl fmt::Display for CompositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "scanout composition: {self:?}")
    }
}
impl Error for CompositionError {}

pub fn compose(scanout: &Scanout<'_>) -> Result<ComposedFrame, CompositionError> {
    dimensions(scanout.width, scanout.height)?;
    let output_bytes = (scanout.width as usize)
        .checked_mul(scanout.height as usize)
        .and_then(|v| v.checked_mul(4))
        .filter(|n| *n <= MAX_BYTES)
        .ok_or(CompositionError::PixelBudget)?;
    if scanout.planes.len() > MAX_PLANES {
        return Err(CompositionError::TooManyPlanes);
    }
    if scanout.planes.is_empty() {
        return Err(CompositionError::PlaneOrder);
    }
    let mut total_source = 0usize;
    for (i, plane) in scanout.planes.iter().enumerate() {
        validate_plane(plane)?;
        total_source = total_source
            .checked_add(plane.pixels.len())
            .filter(|v| *v <= MAX_BYTES)
            .ok_or(CompositionError::PixelBudget)?;
        if plane.id == 0
            || scanout.planes[..i]
                .iter()
                .any(|p| p.id == plane.id || p.z == plane.z)
        {
            return Err(CompositionError::PlaneOrder);
        }
    }
    let mut ordered: Vec<_> = scanout.planes.iter().collect();
    ordered.sort_by_key(|p| p.z);
    let primary = ordered[0];
    if primary.kind != Kind::Primary
        || ordered.iter().filter(|p| p.kind == Kind::Primary).count() != 1
        || primary.opacity != u16::MAX
        || primary.blend != Blend::None
        || primary.destination
            != (Destination {
                x: 0,
                y: 0,
                width: scanout.width,
                height: scanout.height,
            })
    {
        return Err(CompositionError::PrimaryCoverage);
    }
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(output_bytes)
        .map_err(|_| CompositionError::PixelBudget)?;
    pixels.resize(output_bytes, 0);
    for plane in ordered {
        render(plane, scanout.width, scanout.height, &mut pixels)?;
    }
    Ok(ComposedFrame {
        width: scanout.width,
        height: scanout.height,
        pixels_rgba: pixels,
    })
}

fn dimensions(width: u32, height: u32) -> Result<(), CompositionError> {
    if width == 0 || height == 0 || width > MAX_EDGE || height > MAX_EDGE {
        return Err(CompositionError::InvalidDimensions);
    }
    Ok(())
}

fn validate_plane(plane: &Plane<'_>) -> Result<(), CompositionError> {
    dimensions(plane.width, plane.height)?;
    let bytes = (plane.stride as usize)
        .checked_mul(plane.height as usize)
        .filter(|n| *n <= MAX_BYTES)
        .ok_or(CompositionError::PixelBudget)?;
    if plane.stride < plane.width * 4 || plane.pixels.len() != bytes {
        return Err(CompositionError::BufferLayout);
    }
    let crop = plane.crop;
    if crop.width == 0
        || crop.height == 0
        || crop
            .x
            .checked_add(crop.width)
            .is_none_or(|end| end > plane.width)
        || crop
            .y
            .checked_add(crop.height)
            .is_none_or(|end| end > plane.height)
    {
        return Err(CompositionError::SourceBounds);
    }
    let (width, height) = match plane.rotation {
        Rotation::R0 | Rotation::R180 => (crop.width, crop.height),
        Rotation::R90 | Rotation::R270 => (crop.height, crop.width),
    };
    if plane.destination.width != width || plane.destination.height != height {
        return Err(CompositionError::UnsupportedScaling);
    }
    Ok(())
}

fn render(
    plane: &Plane<'_>,
    width: u32,
    height: u32,
    output: &mut [u8],
) -> Result<(), CompositionError> {
    let destination = plane.destination;
    let left = i64::from(destination.x).clamp(0, i64::from(width)) as u32;
    let top = i64::from(destination.y).clamp(0, i64::from(height)) as u32;
    let right =
        (i64::from(destination.x) + i64::from(destination.width)).clamp(0, i64::from(width)) as u32;
    let bottom = (i64::from(destination.y) + i64::from(destination.height))
        .clamp(0, i64::from(height)) as u32;
    for y in top..bottom {
        for x in left..right {
            let dx = (i64::from(x) - i64::from(destination.x)) as u32;
            let dy = (i64::from(y) - i64::from(destination.y)) as u32;
            let (mut sx, mut sy) = match plane.rotation {
                Rotation::R0 => (dx, dy),
                Rotation::R90 => (dy, plane.crop.height - 1 - dx),
                Rotation::R180 => (plane.crop.width - 1 - dx, plane.crop.height - 1 - dy),
                Rotation::R270 => (plane.crop.width - 1 - dy, dx),
            };
            if plane.reflect_x {
                sx = plane.crop.width - 1 - sx;
            }
            if plane.reflect_y {
                sy = plane.crop.height - 1 - sy;
            }
            sx += plane.crop.x;
            sy += plane.crop.y;
            let offset = sy as usize * plane.stride as usize + sx as usize * 4;
            let source = &plane.pixels[offset..offset + 4];
            let rgba = match plane.format {
                PixelFormat::Rgba => [source[0], source[1], source[2], source[3]],
                PixelFormat::Bgra => [source[2], source[1], source[0], source[3]],
                PixelFormat::Bgrx => [source[2], source[1], source[0], 255],
            };
            if plane.blend == Blend::Premultiplied && rgba[..3].iter().any(|c| *c > rgba[3]) {
                return Err(CompositionError::InvalidPremultipliedPixel);
            }
            let alpha = u64::from(if plane.blend == Blend::None {
                255
            } else {
                rgba[3]
            });
            let opacity = u64::from(plane.opacity);
            let denominator = 255u64 * 65535;
            let alpha_product = alpha * opacity;
            let index = (y as usize * width as usize + x as usize) * 4;
            for c in 0..3 {
                let multiplier = if plane.blend == Blend::Premultiplied {
                    255
                } else {
                    alpha
                };
                let numerator = u64::from(rgba[c]) * multiplier * opacity
                    + u64::from(output[index + c]) * (denominator - alpha_product);
                output[index + c] = ((numerator + denominator / 2) / denominator) as u8;
            }
            output[index + 3] = 255;
        }
    }
    Ok(())
}
