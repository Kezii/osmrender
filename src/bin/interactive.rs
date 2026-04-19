use std::{collections::VecDeque, time::Instant};

use embedded_gfx::framebuffer::{DmaReadyFramebuffer, StackFramebuffer};
use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::{DrawTarget, OriginDimensions, PixelColor, Point, RgbColor, Size},
    primitives::Rectangle,
    text::Text,
};
use embedded_graphics_simulator::{
    OutputSettings, SimulatorDisplay, SimulatorEvent, Window, sdl2::Keycode,
};
use log::info;
use osmrender::{
    WorldPos,
    renderprocess::{RenderState, viewport_geo_overscan},
};

const MOUSE_HISTORY_LEN: usize = 4;
const INERTIA_FRICTION_PER_FRAME: f64 = 0.90;
const INERTIA_STOP_SPEED_PX_PER_FRAME: f64 = 0.15;

#[derive(Clone, Copy, Debug, Default)]
struct PanVelocity {
    x: f64,
    y: f64,
}

impl PanVelocity {
    fn from_delta(delta: Point) -> Self {
        Self {
            x: delta.x as f64,
            y: delta.y as f64,
        }
    }

    fn magnitude_sq(self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    fn apply_friction(self, frame_time_secs: f64) -> Option<Self> {
        let frame_scale = (frame_time_secs / (1.0 / 60.0)).max(0.0);
        let damping = INERTIA_FRICTION_PER_FRAME.powf(frame_scale);
        let next = Self {
            x: self.x * damping,
            y: self.y * damping,
        };

        (next.magnitude_sq() >= INERTIA_STOP_SPEED_PX_PER_FRAME.powi(2)).then_some(next)
    }
}

fn push_mouse_history(history: &mut VecDeque<Point>, point: Point) {
    history.push_back(point);
    while history.len() > MOUSE_HISTORY_LEN {
        history.pop_front();
    }
}

fn flick_velocity_from_history(
    history: &VecDeque<Point>,
    release_point: Point,
) -> Option<PanVelocity> {
    let point_two_updates_ago = history.iter().rev().nth(2)?;
    Some(PanVelocity::from_delta(
        release_point - *point_two_updates_ago,
    ))
}

fn geo_delta_per_pixel(render_state: &RenderState, display_size: Size) -> (f64, f64) {
    let pan_scale = viewport_geo_overscan(display_size);
    let lon_per_pixel = ((render_state.bbox.max_lon - render_state.bbox.min_lon)
        / display_size.width as f64)
        * pan_scale;
    let lat_per_pixel = ((render_state.bbox.max_lat - render_state.bbox.min_lat)
        / display_size.height as f64)
        * pan_scale;

    (lat_per_pixel, lon_per_pixel)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let mut display = SimulatorDisplay::<Rgb565>::new(Size::new(1920, 1080));
    let mut window = Window::new("Window", &OutputSettings::default());

    let mut stackframebuffer = StackFramebuffer::<1920, 1080, Rgb565>::new(Rgb565::BLACK);

    let mut centro = WorldPos::new(45.47362, 9.24919);
    let mut raggio_metri = 200.0;
    let mut should_reload = true;

    let text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);

    let mut render_state = RenderState::default();

    let mut click_down_point: Option<Point> = None;
    let mut previous_frame_point: VecDeque<Point> = VecDeque::new();
    let mut center_speed: Option<PanVelocity> = None;

    let mut old_frame = Instant::now();
    'running: loop {
        window.update(&display);

        let now = Instant::now();
        let frame_time = now.duration_since(old_frame);
        old_frame = now;
        let frame_time_secs = frame_time.as_secs_f64();

        if should_reload {
            render_state.set_bbox_for_viewport(centro, raggio_metri, display.size());
            render_state.reload_chunks()?;
            render_state.reload_map_elements()?;
            render_state.reload_mesh_container(&mut display)?;
            should_reload = false;
        }

        for event in window.events() {
            //info!("Event: {:?}", event);
            match event {
                SimulatorEvent::Quit => break 'running,
                SimulatorEvent::KeyDown { keycode, .. } => match keycode {
                    Keycode::Q => {
                        break 'running;
                    }
                    _ => {}
                },
                SimulatorEvent::MouseWheel { scroll_delta, .. } => {
                    raggio_metri -= (raggio_metri / 10.0) * scroll_delta.y as f64;
                    should_reload = true;
                    center_speed = None;
                }
                SimulatorEvent::MouseButtonUp { point, .. } => {
                    if click_down_point.is_some() {
                        push_mouse_history(&mut previous_frame_point, point);
                        center_speed = flick_velocity_from_history(&previous_frame_point, point);
                    }
                    click_down_point = None;
                    previous_frame_point.clear();
                    should_reload = true;
                }
                SimulatorEvent::MouseButtonDown { point, .. } => {
                    click_down_point = Some(point);
                    previous_frame_point.clear();
                    push_mouse_history(&mut previous_frame_point, point);
                    center_speed = None;
                }
                SimulatorEvent::MouseMove { point, .. } => {
                    if let Some(previous_point) = click_down_point {
                        let delta = point - previous_point;
                        let display_size = display.size();
                        let (lat_per_pixel, lon_per_pixel) =
                            geo_delta_per_pixel(&render_state, display_size);

                        // Durante il pan il punto sotto al cursore deve restare lo stesso,
                        // quindi il centro si muove in senso opposto al delta del mouse.
                        // Il fattore di pan include la porzione realmente visibile tramite camera.
                        centro += WorldPos::new(
                            delta.y as f64 * lat_per_pixel,
                            -delta.x as f64 * lon_per_pixel,
                        );
                        click_down_point = Some(point);
                        should_reload = true;
                        center_speed = None;
                        push_mouse_history(&mut previous_frame_point, point);
                    }
                }
                _ => {}
            }
        }

        if let Some(current_speed) = center_speed {
            let display_size = display.size();
            let (lat_per_pixel, lon_per_pixel) = geo_delta_per_pixel(&render_state, display_size);

            centro += WorldPos::new(
                current_speed.y * lat_per_pixel,
                -current_speed.x * lon_per_pixel,
            );

            should_reload = true;
            info!("inertia {:?}", current_speed);
            center_speed = current_speed.apply_friction(frame_time_secs);
        }

        stackframebuffer.clear(Rgb565::BLACK);

        render_state.renderizza_mappa(&mut stackframebuffer);

        Text::new(
            &format!(
                "Frame time: {}ms, fps: {}",
                frame_time.as_millis(),
                1.0 / frame_time.as_secs_f32()
            ),
            Point::new(10, 20),
            text_style,
        )
        .draw(&mut stackframebuffer)?;

        let area = Rectangle::new(Point::new(0, 0), stackframebuffer.size());
        display.fill_contiguous(
            &area,
            stackframebuffer.framebuffer.iter().flatten().copied(),
        );
    }

    Ok(())
}
