use std::time::Instant;

use embedded_graphics::{
    Drawable,
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_6X9, FONT_10X20},
    },
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point, RgbColor, Size},
    text::{Text, TextStyle},
};
use embedded_graphics_simulator::{
    OutputSettings, OutputSettingsBuilder, SimulatorDisplay, SimulatorEvent, Window, sdl2::Keycode,
};
use log::info;
use osmrender::renderprocess::RenderState;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let mut display = SimulatorDisplay::<Rgb565>::new(Size::new(1920, 1080));
    let mut window = Window::new("Window", &OutputSettings::default());

    let centro_lat = 45.47362;
    let centro_lon = 9.24919;
    let raggio_metri = 200.0;

    let text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);

    let mut render_state = RenderState::default();

    render_state.set_bbox(centro_lat, centro_lon, raggio_metri);
    render_state.reload_chunks()?;
    render_state.reload_map_elements()?;
    render_state.reload_mesh_container(&mut display)?;

    let mut old_frame = Instant::now();
    'running: loop {
        window.update(&display);

        for event in window.events() {
            info!("Event: {:?}", event);
            match event {
                SimulatorEvent::Quit => break 'running,
                SimulatorEvent::KeyDown { keycode, .. } => match keycode {
                    Keycode::Q => {
                        break 'running;
                    }
                    _ => {}
                },
                SimulatorEvent::MouseButtonUp { point, .. } => {}
                _ => {}
            }
        }

        let now = Instant::now();
        let frame_time = now.duration_since(old_frame);
        old_frame = now;

        display.clear(Rgb565::BLACK);

        render_state.renderizza_mappa(&mut display)?;

        Text::new(
            &format!(
                "Frame time: {}ms, fps: {}",
                frame_time.as_millis(),
                1.0 / frame_time.as_secs_f32()
            ),
            Point::new(10, 20),
            text_style,
        )
        .draw(&mut display)?;
    }

    Ok(())
}
