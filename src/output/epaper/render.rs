//! Draws one [`Screen`] onto the Waveshare 2.7" tri-color panel.
//!
//! The panel is driven as two 1-bit planes: the crate's `Display2in7b`
//! framebuffer is black/white only (see its `TODO TriColor` note), so red
//! accents are drawn into a second *chromatic* framebuffer — any pixel drawn
//! there shows as red on the panel, taking precedence over black. Both
//! framebuffers are rotated 90° for landscape.
//!
//! profont is latin1-only and has no arrow glyphs, so arrows are drawn as
//! vector shapes instead of text.

use super::layout::{AirportLabel, Body, Route, Screen};
use crate::model::{CompassPoint, VerticalDirection};
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Dimensions, Point},
    mono_font::{MonoFont, MonoTextStyle},
    prelude::Primitive,
    primitives::{Line, PrimitiveStyle, Triangle},
    text::Text,
    Drawable,
};
use epd_waveshare::{color::Color, epd2in7b::Display2in7b, graphics::DisplayRotation};
use profont::{PROFONT_12_POINT, PROFONT_14_POINT, PROFONT_18_POINT, PROFONT_24_POINT, PROFONT_9_POINT};

/// Landscape width after rotation (the native panel is 176x264 portrait).
const PANEL_WIDTH: i32 = 264;
const MARGIN: i32 = 8;

/// Fixed line positions (top y of each line, px). Keeping them fixed — rather
/// than packing upward when fields are missing — keeps the frame stable
/// between refreshes.
const Y_HEADER: i32 = 4; // 24 pt
const Y_RULE: i32 = 36;
const Y_AIRLINE: i32 = 42; // 14 pt
const Y_ROUTE: i32 = 64; // 12 pt
const Y_DETAIL_1: i32 = 86; // 12 pt
const Y_DETAIL_2: i32 = 108; // 12 pt
const Y_TYPE: i32 = 130; // 12 pt

/// Route arrow geometry and the gaps around it.
const ARROW_GAP: i32 = 2;
const ARROW_SHAFT: i32 = 12;
const ARROW_HEAD: i32 = 5;
/// Horizontal space the arrow consumes between the two labels.
const ARROW_TOTAL: i32 = ARROW_GAP + ARROW_SHAFT + ARROW_HEAD + ARROW_GAP;

/// Space reserved for the trend triangle ahead of the altitude text.
const TREND_ADVANCE: i32 = 14;
/// Space reserved for the compass arrow ahead of the heading text.
const HEADING_ADVANCE: i32 = 16;

/// One color plane of a frame.
type Plane = Display2in7b;

/// Both planes of one frame, ready to ship to the panel.
pub struct Planes {
    pub black: Plane,
    pub chromatic: Plane,
}

/// Render a screen to the two pixel planes. Pure: writes RAM buffers only,
/// which is what makes it testable off the Pi.
///
/// Note the framebuffer bit convention: a set bit is an *unwritten* (white)
/// pixel — the driver's `clear` background is `Color::White` — so both planes
/// are cleared to white first, and everything (black text on the black plane,
/// red shapes on the chromatic plane) is drawn "in black", i.e. by clearing
/// bits. The driver inverts the bytes on the wire.
pub fn render(screen: &Screen) -> Planes {
    let mut black = Plane::default();
    let mut chromatic = Plane::default();
    black.set_rotation(DisplayRotation::Rotate90);
    chromatic.set_rotation(DisplayRotation::Rotate90);
    black.clear(Color::White).ok();
    chromatic.clear(Color::White).ok();
    match &screen.body {
        Body::Closest { .. } => render_closest(screen, &mut black, &mut chromatic),
        Body::Empty { radius_km } => render_empty(screen, *radius_km, &mut black),
    }
    Planes { black, chromatic }
}

fn render_closest(screen: &Screen, black: &mut Plane, chromatic: &mut Plane) {
    let Body::Closest {
        callsign,
        airline,
        route,
        altitude,
        trend,
        speed,
        heading,
        distance,
        aircraft_type,
    } = &screen.body
    else {
        unreachable!("closest body checked by caller")
    };

    draw_text(black, callsign, MARGIN, Y_HEADER, &PROFONT_24_POINT);
    let stamp_w = text_width(&screen.stamp, &PROFONT_9_POINT);
    draw_text(
        black,
        &screen.stamp,
        PANEL_WIDTH - MARGIN - stamp_w,
        Y_HEADER + 2,
        &PROFONT_9_POINT,
    );
    draw_rule(black, Y_RULE);

    if let Some(airline) = airline {
        draw_text(black, airline, MARGIN, Y_AIRLINE, &PROFONT_14_POINT);
    }
    if let Some(route) = route {
        draw_route(route, black, chromatic, Y_ROUTE);
    }

    // Detail line 1: altitude (with a red trend triangle) and speed.
    let mut parts: Vec<&str> = Vec::new();
    if let Some(alt) = altitude {
        parts.push(alt);
    }
    if let Some(speed) = speed {
        parts.push(speed);
    }
    let mut x = MARGIN;
    if altitude.is_some()
        && matches!(
            trend,
            Some(VerticalDirection::Ascending) | Some(VerticalDirection::Descending)
        )
    {
        draw_trend_triangle(trend.unwrap(), chromatic, x, Y_DETAIL_1);
        x += TREND_ADVANCE;
    }
    if !parts.is_empty() {
        draw_text(black, &parts.join(" · "), x, Y_DETAIL_1, &PROFONT_12_POINT);
    }

    // Detail line 2: compass arrow and heading, then distance.
    let mut x = MARGIN;
    if let Some(heading) = heading {
        draw_heading_arrow(*heading, black, Point::new(x + 6, Y_DETAIL_2 + 7));
        x += HEADING_ADVANCE;
    }
    let mut parts: Vec<&str> = Vec::new();
    if let Some(heading) = heading {
        parts.push(heading.abbrev());
    }
    parts.push(distance);
    draw_text(black, &parts.join(" · "), x, Y_DETAIL_2, &PROFONT_12_POINT);

    if let Some(kind) = aircraft_type {
        draw_text(black, kind, MARGIN, Y_TYPE, &PROFONT_12_POINT);
    }
}

fn render_empty(screen: &Screen, radius_km: f64, black: &mut Plane) {
    let line1 = "No aircraft within";
    let line2 = format!("{radius_km:.0} km of home");
    let w1 = text_width(line1, &PROFONT_18_POINT);
    let w2 = text_width(&line2, &PROFONT_18_POINT);
    draw_text(black, line1, (PANEL_WIDTH - w1) / 2, 52, &PROFONT_18_POINT);
    draw_text(black, &line2, (PANEL_WIDTH - w2) / 2, 78, &PROFONT_18_POINT);
    let stamp_w = text_width(&screen.stamp, &PROFONT_14_POINT);
    draw_text(black, &screen.stamp, (PANEL_WIDTH - stamp_w) / 2, 112, &PROFONT_14_POINT);
}

/// Labels for the route line: trimmed names when both fit, compact codes
/// otherwise (never a mix — "Athens… → LHR" reads worse than "ATH → LHR").
fn route_labels(origin: &AirportLabel, destination: &AirportLabel) -> (String, String) {
    if let (Some(a), Some(b)) = (&origin.name, &destination.name) {
        let total = MARGIN
            + text_width(a, &PROFONT_12_POINT)
            + ARROW_TOTAL
            + text_width(b, &PROFONT_12_POINT)
            + MARGIN;
        if total <= PANEL_WIDTH {
            return (a.clone(), b.clone());
        }
        return (origin.code.clone(), destination.code.clone());
    }
    (label_text(origin), label_text(destination))
}

/// A single airport: trimmed name when known, compact code otherwise.
fn label_text(label: &AirportLabel) -> String {
    label.name.clone().unwrap_or_else(|| label.code.clone())
}

fn draw_route(route: &Route, black: &mut Plane, chromatic: &mut Plane, y_top: i32) {
    match route {
        Route::Between {
            origin,
            destination,
        } => {
            let (from, to) = route_labels(origin, destination);
            draw_text(black, &from, MARGIN, y_top, &PROFONT_12_POINT);
            let arrow_x = MARGIN + text_width(&from, &PROFONT_12_POINT) + ARROW_GAP;
            draw_route_arrow(chromatic, arrow_x, y_top + 7);
            let to_x = arrow_x + ARROW_SHAFT + ARROW_HEAD + ARROW_GAP;
            draw_text(black, &to, to_x, y_top, &PROFONT_12_POINT);
        }
        Route::Near(label) => {
            draw_text(
                black,
                &format!("near {}", label_text(label)),
                MARGIN,
                y_top,
                &PROFONT_12_POINT,
            );
        }
    }
}

/// A small red right-arrow at (`x`, `cy`), between two route labels.
fn draw_route_arrow(chromatic: &mut Plane, x: i32, cy: i32) {
    let stroke = PrimitiveStyle::with_stroke(Color::Black, 2);
    let shaft_end = x + ARROW_SHAFT;
    Line::new(Point::new(x, cy), Point::new(shaft_end, cy))
        .into_styled(stroke)
        .draw(chromatic)
        .ok();
    for wing in [Point::new(shaft_end, cy - 4), Point::new(shaft_end, cy + 4)] {
        Line::new(wing, Point::new(shaft_end + ARROW_HEAD, cy))
            .into_styled(stroke)
            .draw(chromatic)
            .ok();
    }
}

/// A small filled red triangle marking climb (point up) or descent (down),
/// vertically centered on a 12 pt text line whose top is `y_top`.
fn draw_trend_triangle(trend: VerticalDirection, chromatic: &mut Plane, x: i32, y_top: i32) {
    let cy = y_top + 7;
    let points = match trend {
        VerticalDirection::Ascending => [
            Point::new(x, cy + 4),
            Point::new(x + 9, cy + 4),
            Point::new(x + 4, cy - 5),
        ],
        _ => [
            Point::new(x, cy - 5),
            Point::new(x + 9, cy - 5),
            Point::new(x + 4, cy + 4),
        ],
    };
    Triangle::new(points[0], points[1], points[2])
        .into_styled(PrimitiveStyle::with_fill(Color::Black))
        .draw(chromatic)
        .ok();
}

/// A small compass arrow at `center`, pointing the heading's way. Screen
/// angles: 0° = east (right), 90° = south (down).
fn draw_heading_arrow(heading: CompassPoint, black: &mut Plane, center: Point) {
    let deg: f32 = match heading {
        CompassPoint::East => 0.0,
        CompassPoint::Southeast => 45.0,
        CompassPoint::South => 90.0,
        CompassPoint::Southwest => 135.0,
        CompassPoint::West => 180.0,
        CompassPoint::Northwest => 225.0,
        CompassPoint::North => 270.0,
        CompassPoint::Northeast => 315.0,
    };
    let rad = deg.to_radians();
    let tip = center + Point::new((rad.cos() * 7.0).round() as i32, (rad.sin() * 7.0).round() as i32);
    let tail =
        center - Point::new((rad.cos() * 2.0).round() as i32, (rad.sin() * 2.0).round() as i32);
    let stroke = PrimitiveStyle::with_stroke(Color::Black, 2);
    Line::new(tail, tip).into_styled(stroke).draw(black).ok();
    for off in [150.0, -150.0] {
        let wing_rad = (deg + off).to_radians();
        let wing = tip
            + Point::new((wing_rad.cos() * 5.0).round() as i32, (wing_rad.sin() * 5.0).round() as i32);
        Line::new(tip, wing).into_styled(stroke).draw(black).ok();
    }
}

fn draw_rule(black: &mut Plane, y: i32) {
    Line::new(Point::new(MARGIN, y), Point::new(PANEL_WIDTH - MARGIN, y))
        .into_styled(PrimitiveStyle::with_stroke(Color::Black, 1))
        .draw(black)
        .ok();
}

fn draw_text(plane: &mut Plane, text: &str, x: i32, y_top: i32, font: &'static MonoFont) {
    let style = MonoTextStyle::new(font, Color::Black);
    Text::new(text, Point::new(x, y_top), style).draw(plane).ok();
}

fn text_width(text: &str, font: &'static MonoFont) -> i32 {
    let style = MonoTextStyle::new(font, Color::Black);
    Text::new(text, Point::zero(), style)
        .bounding_box()
        .size
        .width as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AirportRef, FlightInfo, TickResult};
    use crate::output::epaper::layout::{self, AirportLabel, Body, Route, Screen};

    /// Buffer bytes concatenated — what the hardware worker compares to skip
    /// unchanged frames.
    fn frame_bytes(planes: &Planes) -> Vec<u8> {
        let mut v = Vec::with_capacity(planes.black.buffer().len() + planes.chromatic.buffer().len());
        v.extend_from_slice(planes.black.buffer());
        v.extend_from_slice(planes.chromatic.buffer());
        v
    }

    /// A fully populated Closest tick, mirroring the terminal test fixture.
    fn closest() -> TickResult {
        TickResult::Closest {
            flight: Box::new(FlightInfo {
                callsign: "AEE166".into(),
                airline: Some("Aegean Airlines".into()),
                origin: Some(AirportRef {
                    icao: "LGAV".into(),
                    iata: Some("ATH".into()),
                    name: Some("Athens International Airport".into()),
                }),
                destination: Some(AirportRef {
                    icao: "EGLL".into(),
                    iata: Some("LHR".into()),
                    name: Some("London Heathrow".into()),
                }),
                registration: Some("SX-OBN".into()),
                aircraft_type: Some("AT76".into()),
                altitude_ft: Some(6525.0),
                ground_speed_kmh: Some(296.0),
                vertical_direction: Some(VerticalDirection::Descending),
                heading: Some(CompassPoint::Northeast),
                distance_km: 5.6,
            }),
        }
    }

    fn empty() -> TickResult {
        TickResult::Empty { radius_km: 30.0 }
    }

    /// Render via the real layout pipeline but with a fixed stamp, so the
    /// comparison tests don't flake across a minute boundary.
    fn render_fixed(result: &TickResult) -> Planes {
        let mut screen = layout::layout(result);
        screen.stamp = "12:00".into();
        render(&screen)
    }

    #[test]
    fn renders_both_variants_without_panicking() {
        let a = render_fixed(&closest());
        let b = render_fixed(&empty());
        assert_ne!(frame_bytes(&a), frame_bytes(&b));
    }

    #[test]
    fn identical_ticks_render_identically() {
        // The worker's skip-if-unchanged relies on this.
        let a = render_fixed(&closest());
        let b = render_fixed(&closest());
        assert_eq!(frame_bytes(&a), frame_bytes(&b));
    }

    #[test]
    fn closest_uses_both_inks() {
        let planes = render_fixed(&closest());
        // Both planes carry ink: set bits are *white* (cleared) pixels, so a
        // drawn plane differs from the all-white 0xFF background.
        assert!(planes.black.buffer().iter().any(|&b| b != 0xFF));
        assert!(planes.chromatic.buffer().iter().any(|&b| b != 0xFF));
    }

    #[test]
    fn empty_screen_is_black_only() {
        let planes = render_fixed(&empty());
        assert!(planes.black.buffer().iter().any(|&b| b != 0xFF));
        assert!(planes.chromatic.buffer().iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn long_route_names_fall_back_to_codes() {
        let origin = AirportLabel {
            name: Some("San Francisco".into()),
            code: "SFO".into(),
        };
        let destination = AirportLabel {
            name: Some("Los Angeles".into()),
            code: "LAX".into(),
        };
        // Short names are used as-is.
        assert_eq!(route_labels(&origin, &destination), ("San Francisco".into(), "Los Angeles".into()));
        // Overlong names fall back to the codes for both ends.
        let long = AirportLabel {
            name: Some("International Falls".into()),
            code: "INL".into(),
        };
        assert_eq!(route_labels(&long, &long), ("INL".into(), "INL".into()));
    }

    #[test]
    fn screen_positions_hold_for_full_flight() {
        // Every text run must fit the landscape width.
        let screen = Screen {
            stamp: "12:00".into(),
            body: Body::Closest {
                callsign: "AEE166".into(),
                airline: Some("Aegean Airlines".into()),
                route: Some(Route::Between {
                    origin: AirportLabel {
                        name: Some("San Francisco".into()),
                        code: "SFO".into(),
                    },
                    destination: AirportLabel {
                        name: Some("Los Angeles".into()),
                        code: "LAX".into(),
                    },
                }),
                altitude: Some("34000 ft".into()),
                trend: Some(VerticalDirection::Ascending),
                speed: Some("742 km/h".into()),
                heading: Some(CompassPoint::Northeast),
                distance: "5.6 km away".into(),
                aircraft_type: Some("A321".into()),
            },
        };
        let planes = render(&screen);
        assert!(planes.black.buffer().iter().any(|&b| b != 0xFF));
        assert!(planes.chromatic.buffer().iter().any(|&b| b != 0xFF));
    }
}
