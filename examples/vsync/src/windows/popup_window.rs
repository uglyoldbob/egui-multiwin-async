//! This is an example of a popup window. It is likely very crude on the opengl_after function and could probably be optimized
use std::sync::{Arc, Mutex};

use crate::egui_multiwin_dynamic::{
    multi_window::NewWindowRequest,
    tracked_window::{RedrawResponse, TrackedWindow},
};
use egui_multiwin::egui_glow_async::glow;
use egui_multiwin::egui_glow_async::EguiGlow;
use egui_multiwin::egui::containers::panel::AsyncClosure;

use crate::AppCommon;

/// The popup window
pub struct PopupWindow {
    /// The label for the popup window
    pub input: String,
}

impl PopupWindow {
    /// Request a new window
    pub fn request(label: String) -> NewWindowRequest {
        NewWindowRequest::new(
            super::MyWindows::Popup(PopupWindow {
                input: label.clone(),
            }),
            egui_multiwin::async_winit::window::WindowBuilder::new()
                .with_resizable(false)
                .with_inner_size(egui_multiwin::async_winit::dpi::LogicalSize {
                    width: 400.0,
                    height: 200.0,
                })
                .with_title(label),
            egui_multiwin::tracked_window::TrackedWindowOptions {
                vsync: false,
                shader: None,
            },
        )
    }
}

impl TrackedWindow for PopupWindow {
    async unsafe fn opengl_after(
        &mut self,
        _c: &mut AppCommon,
        gl: &std::sync::Arc<egui_multiwin::egui_glow_async::painter::Context>,
    ) {
        use glow::HasContext;
        let shader_version = egui_multiwin::egui_glow_async::ShaderVersion::get(gl);
        let vertex_array = gl
            .create_vertex_array()
            .expect("Cannot create vertex array");
        gl.bind_vertex_array(Some(vertex_array));
        let program = gl.create_program().expect("Cannot create program");
        let (vertex_shader_source, fragment_shader_source) = (
            r#"const vec2 verts[3] = vec2[3](
                vec2(0.5f, 1.0f),
                vec2(0.0f, 0.0f),
                vec2(1.0f, 0.0f)
            );
            out vec2 vert;
            void main() {
                vert = verts[gl_VertexID];
                gl_Position = vec4(vert - 0.5, 0.0, 1.0);
            }"#,
            r#"precision mediump float;
            in vec2 vert;
            out vec4 color;
            void main() {
                color = vec4(vert, 0.5, 1.0);
            }"#,
        );

        let shader_sources = [
            (glow::VERTEX_SHADER, vertex_shader_source),
            (glow::FRAGMENT_SHADER, fragment_shader_source),
        ];
        let mut shaders = Vec::with_capacity(shader_sources.len());
        for (shader_type, shader_source) in shader_sources.iter() {
            let shader = gl
                .create_shader(*shader_type)
                .expect("Cannot create shader");
            gl.shader_source(
                shader,
                &format!(
                    "{}\n{}",
                    shader_version.version_declaration(),
                    shader_source
                ),
            );
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                panic!("{}", gl.get_shader_info_log(shader));
            }
            gl.attach_shader(program, shader);
            shaders.push(shader);
        }
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            panic!("{}", gl.get_program_info_log(program));
        }

        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }

        gl.use_program(Some(program));

        gl.draw_arrays(glow::TRIANGLES, 0, 3);
    }

    fn can_quit(&mut self, c: &mut AppCommon) -> bool {
        (c.clicks & 1) == 0
    }

    async fn redraw(
        &mut self,
        c: &mut AppCommon,
        egui: &mut EguiGlow,
        window: &egui_multiwin::async_winit::window::Window<egui_multiwin::async_winit::ThreadSafe>,
        _clipboard: Arc<Mutex<egui_multiwin::arboard::Clipboard>>,
    ) -> RedrawResponse {
        let quit = Arc::new(Mutex::new(false));
        let quit2 = quit.clone();

        egui_multiwin::egui::CentralPanel::default()
            .show_async(&egui.egui_ctx, |ui| AsyncClosure::new(async move {
                if ui.button("Increment").clicked() {
                    c.clicks += 1;
                    window
                        .set_title(&format!("Title update {}", c.clicks))
                        .await;
                }
                let response = ui.add(egui_multiwin::egui::TextEdit::singleline(&mut self.input));
                if response.changed() {
                    // …
                }
                if response.lost_focus()
                    && ui.input(|i| i.key_pressed(egui_multiwin::egui::Key::Enter))
                {
                    // …
                }
                if ui.button("Quit").clicked() {
                    *quit2.lock().unwrap() = true;
                }
            }))
            .await;
        let quit = *quit.lock().unwrap();
        RedrawResponse {
            quit,
            new_windows: Vec::new(),
        }
    }
}
