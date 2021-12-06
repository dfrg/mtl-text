use cocoa::{appkit::NSView, base::id as cocoa_id};
use objc::{rc::autoreleasepool, runtime::YES};
use winit::platform::macos::WindowExtMacOS;
use winit::{
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoop},
};

const WIDTH: u32 = 1024;
const HEIGHT: u32 = 768;

fn main() {
    let event_loop = EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_inner_size(LogicalSize::new(WIDTH, HEIGHT))
        .with_title("Metal Text")
        .build(&event_loop)
        .unwrap();
    let mut renderer = mtl_text::render::Renderer::new();
    unsafe {
        let view = window.ns_view() as cocoa_id;
        view.setWantsLayer(YES);
        view.setLayer(core::mem::transmute(renderer.layer.as_ref()));
    }
    let size = window.inner_size();
    renderer.set_target_size(size.width, size.height);
    event_loop.run(move |event, _, control_flow| {
        autoreleasepool(|| {
            *control_flow = ControlFlow::Wait;
            use winit::event::Event;
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(size) => {
                        renderer.set_target_size(size.width, size.height);
                    }
                    _ => (),
                },
                Event::MainEventsCleared => {
                    window.request_redraw();
                }
                Event::RedrawRequested(_) => {
                    renderer.render_color([0.2, 1.0, 0.25, 1.0]);
                }
                _ => {}
            }
        });
    });
}
