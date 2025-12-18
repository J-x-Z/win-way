use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use env_logger;
use log::info;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    info!("Starting win-way (Universal Smithay Client)...");

    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("win-way: Native Smithay Backend")
        .build(&event_loop)?;

    info!("Window created successfully. Starting event loop.");

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Wait);

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => elwt.exit(),
            Event::AboutToWait => {
                // Application update code.
                // Queue a RedrawRequested event.
                // window.request_redraw();
            },
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                // Draw here
            },
            _ => ()
        }
    })?;

    Ok(())
}
