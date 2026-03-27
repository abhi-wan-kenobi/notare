#[cfg(target_os = "macos")]
use objc2::AnyThread;
#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSBezierPath, NSColor, NSImage};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSPoint, NSRect, NSSize};

pub enum Overlay {
    Recording,
}

#[cfg(target_os = "macos")]
impl Overlay {
    #[allow(deprecated)]
    pub fn draw(&self, base_image: &NSImage) -> Retained<NSImage> {
        match self {
            Overlay::Recording => draw_recording(base_image),
        }
    }
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn draw_recording(base_image: &NSImage) -> Retained<NSImage> {
    let size = base_image.size();
    let composite_image = NSImage::initWithSize(NSImage::alloc(), size);

    composite_image.lockFocus();

    base_image.drawAtPoint_fromRect_operation_fraction(
        NSPoint::new(0.0, 0.0),
        NSRect::new(NSPoint::new(0.0, 0.0), size),
        objc2_app_kit::NSCompositingOperation::Copy,
        1.0,
    );

    let dot_size = size.width * 0.33;
    let border_width = dot_size * 0.08;
    let dot_x = size.width - dot_size - (size.width * 0.02);
    let dot_y = size.height * 0.02;

    let white_color = NSColor::whiteColor();
    white_color.setFill();

    let outer_rect = NSRect::new(NSPoint::new(dot_x, dot_y), NSSize::new(dot_size, dot_size));
    let outer_path = NSBezierPath::bezierPathWithOvalInRect(outer_rect);
    outer_path.fill();

    let red_color = NSColor::systemRedColor();
    red_color.setFill();

    let red_size = dot_size - (border_width * 2.0);
    let red_x = dot_x + border_width;
    let red_y = dot_y + border_width;
    let red_rect = NSRect::new(NSPoint::new(red_x, red_y), NSSize::new(red_size, red_size));
    let red_path = NSBezierPath::bezierPathWithOvalInRect(red_rect);
    red_path.fill();

    let center_size = red_size * 0.45;
    let center_x = red_x + (red_size - center_size) / 2.0;
    let center_y = red_y + (red_size - center_size) / 2.0;

    white_color.setFill();
    let center_rect = NSRect::new(
        NSPoint::new(center_x, center_y),
        NSSize::new(center_size, center_size),
    );
    let center_path = NSBezierPath::bezierPathWithOvalInRect(center_rect);
    center_path.fill();

    composite_image.unlockFocus();

    composite_image
}
