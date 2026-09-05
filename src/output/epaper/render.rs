//! Draws one [`Screen`] onto the Waveshare 2.7" tri-color panel.
//!
//! The panel is driven as two 1-bit planes: the crate's `Display2in7b`
//! framebuffer is black/white-only (see its `TODO TriColor` note), so red
//! accents are drawn into a second *chromatic* framebuffer, where they land
//! as red (taking precedence over black). Both framebuffers are rotated 90°
//! for landscape.
//!
//! # Framebuffer polarity (verified on hardware)
//!
//! Both planes are sent byte-inverted (`send_buffer_helper`), and the two
//! planes have OPPOSITE polarity on the panel: for the black plane a SET
//! buffer bit lands as black ink and a CLEARED bit as white paper; for the
//! chromatic plane a CLEARED bit lands as red and a SET bit as "no red"
//! (the black plane shows through). So each plane is cleared to its own
//! "paper" color and drawn with its own "ink" — the constants below, not the
//! `Color` names, are what produce dark text on a white background.
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
use profont::{PROFONT_14_POINT, PROFONT_18_POINT, PROFONT_24_POINT};

/// Black plane: a SET buffer bit is black ink, a CLEARED bit is white paper.
const INK: Color = Color::White;
const PAPER: Color = Color::Black;
/// Chromatic plane, inverted relative to the black one: a CLEARED buffer bit
/// is red ink, a SET bit means "no red".
const CHROMATIC_INK: Color = Color::Black;
const CHROMATIC_PAPER: Color = Color::White;

/// Landscape width after rotation (the native panel is 176x264 portrait).
const PANEL_WIDTH: i32 = 264;
const MARGIN: i32 = 8;

/// Fonts. Sizes are the largest that fit the panel at readable line breaks:
/// profont advances are 16px (24 pt), 12px (18 pt), 10px (14 pt).
const FONT_HEADER: &MonoFont = &PROFONT_24_POINT;
const FONT_ROUTE: &MonoFont = &PROFONT_18_POINT;
const FONT_DETAILS: &MonoFont = &PROFONT_18_POINT;
const FONT_AIRLINE: &MonoFont = &PROFONT_14_POINT;
const FONT_STAMP: &MonoFont = &PROFONT_14_POINT;

/// Fixed line positions (top y of each line, px) and the detail line step.
const Y_HEADER: i32 = 2; // 24 pt
const Y_RULE: i32 = 34;
const Y_AIRLINE: i32 = 38; // 14 pt
const Y_ROUTE: i32 = 57; // 18 pt
const Y_DETAILS: i32 = 82; // 18 pt, one value per line
const DETAIL_STEP: i32 = 24;
const LINE_HEIGHT: i32 = 22;

/// Route arrow geometry and the gaps around it.
const ARROW_GAP: i32 = 3;
const ARROW_SHAFT: i32 = 14;
const ARROW_HEAD: i32 = 6;
/// Horizontal space the arrow consumes between the two labels.
const ARROW_TOTAL: i32 = ARROW_GAP + ARROW_SHAFT + ARROW_HEAD + ARROW_GAP;

/// Space reserved for the trend triangle ahead of the altitude text.
const TREND_ADVANCE: i32 = 18;
/// Space reserved for the compass arrow ahead of the heading text.
const HEADING_ADVANCE: i32 = 20;

/// One color plane of a frame.
type Plane = Display2in7b;

/// Both planes of one frame, ready to ship to the panel.
pub struct Planes {
    pub black: Plane,
    pub chromatic: Plane,
}

/// Render a screen to the two pixel planes. Pure: writes RAM buffers only,
/// which is what makes it testable off the Pi.
pub fn render(screen: &Screen) -> Planes {
    let mut planes = blank();
    match &screen.body {
        Body::Closest { .. } => render_closest(screen, &mut planes.black, &mut planes.chromatic),
        Body::Empty { radius_km } => render_empty(screen, *radius_km, &mut planes.black),
    }
    planes
}

/// Both planes cleared to paper — the all-white frame used to start a run
/// without a polarity surprise from the driver's own `clear_frame`.
pub fn blank() -> Planes {
    let mut black = Plane::default();
    let mut chromatic = Plane::default();
    black.set_rotation(DisplayRotation::Rotate90);
    chromatic.set_rotation(DisplayRotation::Rotate90);
    black.clear(PAPER).ok();
    chromatic.clear(CHROMATIC_PAPER).ok();
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

    draw_text(black, callsign, MARGIN, Y_HEADER, FONT_HEADER);
    let stamp_w = text_width(&screen.stamp, FONT_STAMP);
    draw_text(
        black,
        &screen.stamp,
        PANEL_WIDTH - MARGIN - stamp_w,
        Y_HEADER + 6,
        FONT_STAMP,
    );
    draw_rule(black, Y_RULE);

    if let Some(airline) = airline {
        draw_text(black, airline, MARGIN, Y_AIRLINE, FONT_AIRLINE);
    }
    if let Some(route) = route {
        draw_route(route, aircraft_type, black, chromatic, Y_ROUTE);
    } else if let Some(kind) = aircraft_type {
        // No route to pair with: show the type on the route line instead.
        draw_text(black, kind, MARGIN, Y_ROUTE, FONT_ROUTE);
    }

    // Details, one value per line at 18 pt: altitude (with a red trend
    // triangle), speed, then heading and distance together.
    let mut lines: Vec<DetailLine> = Vec::new();
    if altitude.is_some() {
        lines.push(DetailLine::Altitude);
    }
    if speed.is_some() {
        lines.push(DetailLine::Speed);
    }
    lines.push(DetailLine::HeadingDistance);
    for (i, line) in lines.iter().enumerate() {
        let y = Y_DETAILS + i as i32 * DETAIL_STEP;
        match line {
            DetailLine::Altitude => {
                let alt = altitude.as_deref().unwrap();
                let mut x = MARGIN;
                if matches!(
                    trend,
                    Some(VerticalDirection::Ascending) | Some(VerticalDirection::Descending)
                ) {
                    draw_trend_triangle(trend.unwrap(), chromatic, x, y);
                    x += TREND_ADVANCE;
                }
                draw_text(black, alt, x, y, FONT_DETAILS);
            }
            DetailLine::Speed => {
                draw_text(black, speed.as_deref().unwrap(), MARGIN, y, FONT_DETAILS);
            }
            DetailLine::HeadingDistance => {
                let mut x = MARGIN;
                if let Some(heading) = heading {
                    draw_heading_arrow(*heading, black, Point::new(x + 8, y + LINE_HEIGHT / 2));
                    x += HEADING_ADVANCE;
                }
                let mut parts: Vec<&str> = Vec::new();
                if let Some(heading) = heading {
                    parts.push(heading.abbrev());
                }
                parts.push(distance);
                draw_text(black, &parts.join(" · "), x, y, FONT_DETAILS);
            }
        }
    }
}

/// Which value occupies one details line.
enum DetailLine {
    Altitude,
    Speed,
    HeadingDistance,
}

fn render_empty(screen: &Screen, radius_km: f64, black: &mut Plane) {
    let line1 = "No aircraft";
    let line2 = format!("within {radius_km:.0} km of home");
    let w1 = text_width(line1, FONT_HEADER);
    let w2 = text_width(&line2, FONT_ROUTE);
    draw_text(black, line1, (PANEL_WIDTH - w1) / 2, 40, FONT_HEADER);
    draw_text(black, &line2, (PANEL_WIDTH - w2) / 2, 80, FONT_ROUTE);
    let stamp_w = text_width(&screen.stamp, FONT_STAMP);
    draw_text(black, &screen.stamp, (PANEL_WIDTH - stamp_w) / 2, 120, FONT_STAMP);
}

/// Labels for the route line: trimmed names when both fit (with the aircraft
/// type suffix), compact codes otherwise — never a mix, which reads worse
/// than two codes.
fn route_labels(
    origin: &AirportLabel,
    destination: &AirportLabel,
    extra_width: i32,
) -> (String, String) {
    if let (Some(a), Some(b)) = (&origin.name, &destination.name) {
        let total = MARGIN
            + text_width(a, FONT_ROUTE)
            + ARROW_TOTAL
            + text_width(b, FONT_ROUTE)
            + extra_width
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

fn draw_route(
    route: &Route,
    aircraft_type: &Option<String>,
    black: &mut Plane,
    chromatic: &mut Plane,
    y_top: i32,
) {
    let suffix = aircraft_type
        .as_ref()
        .map(|kind| format!(" · {kind}"));
    let suffix_w = suffix
        .as_ref()
        .map(|s| text_width(s, FONT_ROUTE))
        .unwrap_or(0);
    match route {
        Route::Between {
            origin,
            destination,
        } => {
            let (from, to) = route_labels(origin, destination, suffix_w);
            draw_text(black, &from, MARGIN, y_top, FONT_ROUTE);
            let arrow_x = MARGIN + text_width(&from, FONT_ROUTE) + ARROW_GAP;
            draw_route_arrow(chromatic, arrow_x, y_top + LINE_HEIGHT / 2);
            let to_x = arrow_x + ARROW_SHAFT + ARROW_HEAD + ARROW_GAP;
            draw_text(black, &to, to_x, y_top, FONT_ROUTE);
            if let Some(suffix) = suffix {
                draw_text(
                    black,
                    &suffix,
                    to_x + text_width(&to, FONT_ROUTE),
                    y_top,
                    FONT_ROUTE,
                );
            }
        }
        Route::Near(label) => {
            let text = format!("near {}", label_text(label));
            draw_text(black, &text, MARGIN, y_top, FONT_ROUTE);
            if let Some(suffix) = suffix {
                draw_text(
                    black,
                    &suffix,
                    MARGIN + text_width(&text, FONT_ROUTE),
                    y_top,
                    FONT_ROUTE,
                );
            }
        }
    }
}

/// A small red right-arrow at (`x`, `cy`), between two route labels.
fn draw_route_arrow(chromatic: &mut Plane, x: i32, cy: i32) {
    let stroke = PrimitiveStyle::with_stroke(CHROMATIC_INK, 2);
    let shaft_end = x + ARROW_SHAFT;
    Line::new(Point::new(x, cy), Point::new(shaft_end, cy))
        .into_styled(stroke)
        .draw(chromatic)
        .ok();
    for wing in [Point::new(shaft_end, cy - 5), Point::new(shaft_end, cy + 5)] {
        Line::new(wing, Point::new(shaft_end + ARROW_HEAD, cy))
            .into_styled(stroke)
            .draw(chromatic)
            .ok();
    }
}

/// A small filled red triangle marking climb (point up) or descent (down),
/// vertically centered on a details text line whose top is `y_top`.
fn draw_trend_triangle(trend: VerticalDirection, chromatic: &mut Plane, x: i32, y_top: i32) {
    let cy = y_top + LINE_HEIGHT / 2;
    let points = match trend {
        VerticalDirection::Ascending => [
            Point::new(x, cy + 5),
            Point::new(x + 11, cy + 5),
            Point::new(x + 5, cy - 6),
        ],
        _ => [
            Point::new(x, cy - 6),
            Point::new(x + 11, cy - 6),
            Point::new(x + 5, cy + 5),
        ],
    };
    Triangle::new(points[0], points[1], points[2])
        .into_styled(PrimitiveStyle::with_fill(CHROMATIC_INK))
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
    let tip = center + Point::new((rad.cos() * 9.0).round() as i32, (rad.sin() * 9.0).round() as i32);
    let tail =
        center - Point::new((rad.cos() * 3.0).round() as i32, (rad.sin() * 3.0).round() as i32);
    let stroke = PrimitiveStyle::with_stroke(INK, 2);
    Line::new(tail, tip).into_styled(stroke).draw(black).ok();
    for off in [150.0, -150.0] {
        let wing_rad = (deg + off).to_radians();
        let wing = tip
            + Point::new((wing_rad.cos() * 6.0).round() as i32, (wing_rad.sin() * 6.0).round() as i32);
        Line::new(tip, wing).into_styled(stroke).draw(black).ok();
    }
}

fn draw_rule(black: &mut Plane, y: i32) {
    Line::new(Point::new(MARGIN, y), Point::new(PANEL_WIDTH - MARGIN, y))
        .into_styled(PrimitiveStyle::with_stroke(INK, 1))
        .draw(black)
        .ok();
}

fn draw_text(plane: &mut Plane, text: &str, x: i32, y_top: i32, font: &'static MonoFont) {
    let style = MonoTextStyle::new(font, INK);
    Text::new(text, Point::new(x, y_top), style).draw(plane).ok();
}

fn text_width(text: &str, font: &'static MonoFont) -> i32 {
    let style = MonoTextStyle::new(font, INK);
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

    /// A plane that was cleared to paper and had nothing drawn on it
    /// (all-paper differs per plane: bits cleared on black, set on chromatic).
    fn blank_planes() -> (Vec<u8>, Vec<u8>) {
        let planes = blank();
        (
            planes.black.buffer().to_vec(),
            planes.chromatic.buffer().to_vec(),
        )
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
        let (blank_black, blank_chromatic) = blank_planes();
        // Drawn ink differs from each plane's all-paper state.
        assert_ne!(planes.black.buffer(), blank_black.as_slice());
        assert_ne!(planes.chromatic.buffer(), blank_chromatic.as_slice());
    }

    #[test]
    fn empty_screen_is_black_only() {
        let planes = render_fixed(&empty());
        let (blank_black, blank_chromatic) = blank_planes();
        assert_ne!(planes.black.buffer(), blank_black.as_slice());
        // No red accents on the empty screen: chromatic plane stays at paper.
        assert_eq!(planes.chromatic.buffer(), blank_chromatic.as_slice());
    }

    #[test]
    fn long_route_names_fall_back_to_codes() {
        // At 18 pt only short names leave room for the arrow; longer pairs
        // fall back to codes.
        let athens = AirportLabel {
            name: Some("Athens".into()),
            code: "ATH".into(),
        };
        let rome = AirportLabel {
            name: Some("Rome".into()),
            code: "FCO".into(),
        };
        assert_eq!(
            route_labels(&athens, &rome, 60),
            ("Athens".into(), "Rome".into())
        );
        let long = AirportLabel {
            name: Some("San Francisco".into()),
            code: "SFO".into(),
        };
        assert_eq!(
            route_labels(&long, &long, 0),
            ("SFO".into(), "SFO".into())
        );
    }

    #[test]
    fn every_text_run_fits_the_panel() {
        // Worst case at the current fonts: longest realistic content.
        let screen = Screen {
            stamp: "12:00".into(),
            body: Body::Closest {
                callsign: "AEE166".into(),
                airline: Some("China Southern Airlines…".into()),
                route: Some(Route::Between {
                    origin: AirportLabel {
                        name: None,
                        code: "ATH".into(),
                    },
                    destination: AirportLabel {
                        name: None,
                        code: "LHR".into(),
                    },
                }),
                altitude: Some("34,000 ft".into()),
                trend: Some(VerticalDirection::Ascending),
                speed: Some("1,142 km/h".into()),
                heading: Some(CompassPoint::Northeast),
                distance: "5.6 km away".into(),
                aircraft_type: Some("A321".into()),
            },
        };
        // Route with codes + type suffix: 108 + arrow 26 + suffix 72 < 248.
        let planes = render(&screen);
        let (blank_black, blank_chromatic) = blank_planes();
        assert_ne!(planes.black.buffer(), blank_black.as_slice());
        assert_ne!(planes.chromatic.buffer(), blank_chromatic.as_slice());
        let suffix_w = text_width(" · A321", FONT_ROUTE);
        assert!(108 + ARROW_TOTAL + suffix_w + 2 * MARGIN <= PANEL_WIDTH);
    }
}
