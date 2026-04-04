use std::path::Path;

use anyhow::{Context, Result};
use eframe::egui;
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputInfo, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_region, wl_shm, wl_surface},
    Connection, Dispatch, QueueHandle,
};

use phantom::overlay::{OverlayFrame, OverlayPreviewSnapshot};
use phantom::profile::{JoystickMode, MouseCameraActivationMode, Node, Profile, Region, RelPos};

const OVERLAY_BG: Color32 = Color32::from_rgb(11, 13, 18);
const MARKER_RADIUS: f32 = 14.0;
const SMALL_MARKER_RADIUS: f32 = 9.0;
const TEXT_SHADOW: Color32 = Color32::from_black_alpha(190);
const GUIDE_LENGTH: f32 = 14.0;
const HEADER_TEXT: &str = "Experimental debug preview — not gameplay-safe";

const MARKER_SIZE: u32 = 64;
const DOT_SIZE: u32 = 20;
const GUIDE_SIZE: u32 = 22;
const JOYSTICK_PADDING: f32 = 34.0;

#[derive(Clone, Copy)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

#[derive(Clone, Copy)]
struct PixelRect {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
}

impl PixelRect {
    fn center(self) -> (f32, f32) {
        (
            self.left as f32 + self.width as f32 * 0.5,
            self.top as f32 + self.height as f32 * 0.5,
        )
    }
}

#[derive(Clone)]
struct MarkerSpec {
    rect: PixelRect,
    radius: f32,
    stroke: Rgba,
    fill: Rgba,
    label: String,
}

#[derive(Clone)]
struct FixedJoystickSpec {
    rect: PixelRect,
    radius: f32,
    keys: [String; 4],
    color: Rgba,
}

#[derive(Clone, Copy)]
enum GuideCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone)]
struct CornerGuideSpec {
    rect: PixelRect,
    color: Rgba,
    corner: GuideCorner,
}

#[derive(Clone)]
struct DotSpec {
    rect: PixelRect,
    radius: f32,
    color: Rgba,
}

#[derive(Clone)]
enum PreviewSpec {
    Marker(MarkerSpec),
    FixedJoystick(FixedJoystickSpec),
    CornerGuide(CornerGuideSpec),
    Dot(DotSpec),
}

impl PreviewSpec {
    fn rect(&self) -> PixelRect {
        match self {
            Self::Marker(spec) => spec.rect,
            Self::FixedJoystick(spec) => spec.rect,
            Self::CornerGuide(spec) => spec.rect,
            Self::Dot(spec) => spec.rect,
        }
    }
}

pub fn run_overlay(snapshot_path: &Path) -> eframe::Result<()> {
    let snapshot = load_preview_snapshot(snapshot_path).map_err(map_anyhow_to_eframe)?;

    if (std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var_os("WAYLAND_SOCKET").is_some())
        && snapshot.frame.is_some()
    {
        match run_wayland_overlay(&snapshot) {
            Ok(()) => return Ok(()),
            Err(e) => {
                tracing::warn!(
                    "wayland preview overlay unavailable, falling back to legacy window preview: {}",
                    e
                );
            }
        }
    }

    run_legacy_overlay(snapshot.profile)
}

fn map_anyhow_to_eframe(error: anyhow::Error) -> eframe::Error {
    eframe::Error::AppCreation(Box::new(std::io::Error::other(error.to_string())))
}

fn load_preview_snapshot(path: &Path) -> Result<OverlayPreviewSnapshot> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read overlay snapshot {}", path.display()))?;
    if let Ok(snapshot) = serde_json::from_slice::<OverlayPreviewSnapshot>(&bytes) {
        return Ok(snapshot);
    }
    let profile = serde_json::from_slice::<Profile>(&bytes)
        .with_context(|| format!("failed to parse overlay snapshot {}", path.display()))?;
    Ok(OverlayPreviewSnapshot {
        profile,
        frame: None,
    })
}

fn run_wayland_overlay(snapshot: &OverlayPreviewSnapshot) -> Result<()> {
    let Some(frame) = snapshot.frame else {
        return Err(anyhow::anyhow!(
            "overlay snapshot has no host frame for Wayland preview placement"
        ));
    };

    let conn = Connection::connect_to_env().context("failed to connect to Wayland compositor")?;
    let (globals, mut event_queue) =
        registry_queue_init(&conn).context("failed to initialize Wayland registry")?;
    let qh = event_queue.handle();

    let mut app = PreviewLayerApp {
        registry_state: RegistryState::new(&globals),
        compositor_state: CompositorState::bind(&globals, &qh)
            .context("wl_compositor not available")?,
        output_state: OutputState::new(&globals, &qh),
        shm: Shm::bind(&globals, &qh).context("wl_shm not available")?,
        layer_shell: LayerShell::bind(&globals, &qh).context("layer shell not available")?,
        frame,
        specs: build_preview_specs(&snapshot.profile, frame),
        surfaces: Vec::new(),
        exit: false,
    };

    event_queue
        .roundtrip(&mut app)
        .context("failed to collect initial Wayland globals")?;
    app.create_surfaces(&qh)?;
    event_queue
        .roundtrip(&mut app)
        .context("failed to configure preview overlay surfaces")?;

    while !app.exit {
        event_queue
            .blocking_dispatch(&mut app)
            .context("preview overlay Wayland dispatch failed")?;
    }

    Ok(())
}

#[derive(Clone, Copy)]
struct OutputGeometry {
    logical_x: i32,
    logical_y: i32,
    logical_width: i32,
    logical_height: i32,
}

struct PreviewSurface {
    layer: LayerSurface,
    pool: SlotPool,
    spec: PreviewSpec,
    width: u32,
    height: u32,
    configured: bool,
}

struct PreviewLayerApp {
    registry_state: RegistryState,
    compositor_state: CompositorState,
    output_state: OutputState,
    shm: Shm,
    layer_shell: LayerShell,
    frame: OverlayFrame,
    specs: Vec<PreviewSpec>,
    surfaces: Vec<PreviewSurface>,
    exit: bool,
}

impl PreviewLayerApp {
    fn create_surfaces(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        self.surfaces.clear();

        let outputs: Vec<_> = self
            .output_state
            .outputs()
            .filter_map(|output| {
                self.output_state
                    .info(&output)
                    .map(|info| (output, output_geometry(&info)))
            })
            .collect();
        if outputs.is_empty() {
            return Err(anyhow::anyhow!(
                "no Wayland outputs available for preview overlay"
            ));
        }

        for spec in &self.specs {
            let rect = spec.rect();
            let Some((output, geometry)) = select_output_for_rect(rect, &outputs) else {
                continue;
            };

            let surface = self.compositor_state.create_surface(qh);
            let layer = self.layer_shell.create_layer_surface(
                qh,
                surface,
                Layer::Overlay,
                Some("phantom-preview-overlay"),
                Some(output),
            );
            layer.set_anchor(Anchor::TOP | Anchor::LEFT);
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            layer.set_exclusive_zone(-1);
            layer.set_size(rect.width, rect.height);
            layer.set_margin(
                (rect.top - geometry.logical_y).max(0),
                0,
                0,
                (rect.left - geometry.logical_x).max(0),
            );

            let region = self.compositor_state.wl_compositor().create_region(qh, ());
            layer.set_input_region(Some(&region));
            region.destroy();
            layer.commit();

            let pool = SlotPool::new((rect.width * rect.height * 4) as usize, &self.shm)
                .context("failed to create preview overlay shm pool")?;

            self.surfaces.push(PreviewSurface {
                layer,
                pool,
                spec: spec.clone(),
                width: rect.width,
                height: rect.height,
                configured: false,
            });
        }

        if self.surfaces.is_empty() {
            return Err(anyhow::anyhow!(
                "preview overlay has no surfaces inside visible outputs (frame={}x{} at {}, {})",
                self.frame.width,
                self.frame.height,
                self.frame.left,
                self.frame.top
            ));
        }

        Ok(())
    }

    fn draw_surface(&mut self, index: usize) {
        let Some(surface) = self.surfaces.get_mut(index) else {
            return;
        };
        if !surface.configured {
            return;
        }

        let stride = surface.width as i32 * 4;
        let (buffer, canvas) = match surface.pool.create_buffer(
            surface.width as i32,
            surface.height as i32,
            stride,
            wl_shm::Format::Argb8888,
        ) {
            Ok(buffer) => buffer,
            Err(e) => {
                tracing::warn!("preview overlay buffer allocation failed: {}", e);
                return;
            }
        };

        draw_preview_spec(canvas, surface.width, surface.height, &surface.spec);

        surface
            .layer
            .wl_surface()
            .damage_buffer(0, 0, surface.width as i32, surface.height as i32);
        buffer
            .attach_to(surface.layer.wl_surface())
            .expect("preview overlay buffer attach");
        surface.layer.commit();
    }
}

impl CompositorHandler for PreviewLayerApp {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for PreviewLayerApp {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if let Some(index) = self
            .surfaces
            .iter()
            .position(|surface| surface.layer == *layer)
        {
            let surface = &mut self.surfaces[index];
            if configure.new_size.0 != 0 && configure.new_size.1 != 0 {
                surface.width = configure.new_size.0.max(surface.width);
                surface.height = configure.new_size.1.max(surface.height);
            }
            surface.configured = true;
            self.draw_surface(index);
        }
    }
}

impl OutputHandler for PreviewLayerApp {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl ShmHandler for PreviewLayerApp {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_compositor!(PreviewLayerApp);
delegate_output!(PreviewLayerApp);
delegate_shm!(PreviewLayerApp);
delegate_layer!(PreviewLayerApp);
delegate_registry!(PreviewLayerApp);

impl ProvidesRegistryState for PreviewLayerApp {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState];
}

impl Dispatch<wl_region::WlRegion, ()> for PreviewLayerApp {
    fn event(
        _: &mut Self,
        _: &wl_region::WlRegion,
        _: wl_region::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

fn build_preview_specs(profile: &Profile, frame: OverlayFrame) -> Vec<PreviewSpec> {
    let mut specs = Vec::new();
    for node in &profile.nodes {
        match node {
            Node::Tap { pos, .. }
            | Node::HoldTap { pos, .. }
            | Node::ToggleTap { pos, .. }
            | Node::RepeatTap { pos, .. } => {
                specs.push(PreviewSpec::Marker(button_marker_spec(
                    frame,
                    pos,
                    display_label(&compact_label(node)),
                    marker_color(node),
                )));
            }
            Node::Wheel {
                up_pos, down_pos, ..
            } => {
                specs.push(PreviewSpec::Marker(button_marker_spec(
                    frame,
                    up_pos,
                    "Up".into(),
                    wheel_color(),
                )));
                specs.push(PreviewSpec::Marker(button_marker_spec(
                    frame,
                    down_pos,
                    "Dn".into(),
                    wheel_color(),
                )));
            }
            Node::Joystick {
                mode: JoystickMode::Fixed,
                pos,
                radius,
                keys,
                ..
            } => {
                specs.push(PreviewSpec::FixedJoystick(fixed_joystick_spec(
                    frame, pos, *radius, keys,
                )));
            }
            Node::Joystick {
                mode: JoystickMode::Floating,
                region: Some(region),
                ..
            } => {
                let zone = frame_region(frame, region);
                specs.extend(floating_joystick_specs(zone));
            }
            Node::Drag {
                start, end, key, ..
            } => {
                specs.push(PreviewSpec::Marker(button_marker_spec(
                    frame,
                    start,
                    display_label(key),
                    drag_color(),
                )));
                let (start_x, start_y) = frame_point(frame, start);
                let (end_x, end_y) = frame_point(frame, end);
                for step in 1..=4 {
                    let t = step as f32 / 5.0;
                    specs.push(PreviewSpec::Dot(dot_spec(
                        lerp(start_x, end_x, t),
                        lerp(start_y, end_y, t),
                        4.0,
                        drag_color(),
                    )));
                }
                specs.push(PreviewSpec::Dot(dot_spec(end_x, end_y, 6.0, drag_color())));
            }
            Node::MouseCamera {
                anchor,
                activation_mode,
                ..
            } => {
                specs.push(PreviewSpec::Marker(button_marker_spec(
                    frame,
                    anchor,
                    aim_label(activation_mode.clone()),
                    aim_color(),
                )));
            }
            Node::Macro { .. } | Node::LayerShift { .. } => {}
            Node::Joystick {
                mode: JoystickMode::Floating,
                region: None,
                ..
            } => {}
        }
    }
    specs
}

fn button_marker_spec(frame: OverlayFrame, pos: &RelPos, label: String, color: Rgba) -> MarkerSpec {
    let (x, y) = frame_point(frame, pos);
    let left = (x - MARKER_SIZE as f32 * 0.5).round() as i32;
    let top = (y - MARKER_SIZE as f32 * 0.5).round() as i32;
    MarkerSpec {
        rect: PixelRect {
            left,
            top,
            width: MARKER_SIZE,
            height: MARKER_SIZE,
        },
        radius: 17.0,
        stroke: color,
        fill: Rgba {
            r: color.r,
            g: color.g,
            b: color.b,
            a: 36,
        },
        label,
    }
}

fn fixed_joystick_spec(
    frame: OverlayFrame,
    pos: &RelPos,
    radius: f64,
    keys: &phantom::profile::JoystickKeys,
) -> FixedJoystickSpec {
    let (x, y) = frame_point(frame, pos);
    let radius_px = (frame.width.min(frame.height) * radius as f32).max(22.0);
    let extent = (radius_px + JOYSTICK_PADDING).ceil() as i32;
    let side = (extent * 2) as u32;
    FixedJoystickSpec {
        rect: PixelRect {
            left: x.round() as i32 - extent,
            top: y.round() as i32 - extent,
            width: side,
            height: side,
        },
        radius: radius_px,
        keys: [
            display_label(&keys.up),
            display_label(&keys.left),
            display_label(&keys.right),
            display_label(&keys.down),
        ],
        color: joystick_color(),
    }
}

fn floating_joystick_specs(zone: PixelRect) -> Vec<PreviewSpec> {
    let color = joystick_color();
    vec![
        PreviewSpec::CornerGuide(CornerGuideSpec {
            rect: PixelRect {
                left: zone.left - 1,
                top: zone.top - 1,
                width: GUIDE_SIZE,
                height: GUIDE_SIZE,
            },
            color,
            corner: GuideCorner::TopLeft,
        }),
        PreviewSpec::CornerGuide(CornerGuideSpec {
            rect: PixelRect {
                left: zone.left + zone.width as i32 - GUIDE_SIZE as i32 + 1,
                top: zone.top - 1,
                width: GUIDE_SIZE,
                height: GUIDE_SIZE,
            },
            color,
            corner: GuideCorner::TopRight,
        }),
        PreviewSpec::CornerGuide(CornerGuideSpec {
            rect: PixelRect {
                left: zone.left - 1,
                top: zone.top + zone.height as i32 - GUIDE_SIZE as i32 + 1,
                width: GUIDE_SIZE,
                height: GUIDE_SIZE,
            },
            color,
            corner: GuideCorner::BottomLeft,
        }),
        PreviewSpec::CornerGuide(CornerGuideSpec {
            rect: PixelRect {
                left: zone.left + zone.width as i32 - GUIDE_SIZE as i32 + 1,
                top: zone.top + zone.height as i32 - GUIDE_SIZE as i32 + 1,
                width: GUIDE_SIZE,
                height: GUIDE_SIZE,
            },
            color,
            corner: GuideCorner::BottomRight,
        }),
        PreviewSpec::Marker(MarkerSpec {
            rect: PixelRect {
                left: zone.left + zone.width as i32 / 2 - MARKER_SIZE as i32 / 2,
                top: zone.top + zone.height as i32 / 2 - MARKER_SIZE as i32 / 2,
                width: MARKER_SIZE,
                height: MARKER_SIZE,
            },
            radius: 15.0,
            stroke: color,
            fill: Rgba {
                r: color.r,
                g: color.g,
                b: color.b,
                a: 28,
            },
            label: "Move".into(),
        }),
    ]
}

fn dot_spec(x: f32, y: f32, radius: f32, color: Rgba) -> DotSpec {
    DotSpec {
        rect: PixelRect {
            left: (x - DOT_SIZE as f32 * 0.5).round() as i32,
            top: (y - DOT_SIZE as f32 * 0.5).round() as i32,
            width: DOT_SIZE,
            height: DOT_SIZE,
        },
        radius,
        color,
    }
}

fn frame_point(frame: OverlayFrame, pos: &RelPos) -> (f32, f32) {
    (
        frame.left + frame.width * pos.x as f32,
        frame.top + frame.height * pos.y as f32,
    )
}

fn frame_region(frame: OverlayFrame, region: &Region) -> PixelRect {
    PixelRect {
        left: (frame.left + frame.width * region.x as f32).round() as i32,
        top: (frame.top + frame.height * region.y as f32).round() as i32,
        width: (frame.width * region.w as f32).max(1.0).round() as u32,
        height: (frame.height * region.h as f32).max(1.0).round() as u32,
    }
}

fn select_output_for_rect(
    rect: PixelRect,
    outputs: &[(wl_output::WlOutput, OutputGeometry)],
) -> Option<(&wl_output::WlOutput, &OutputGeometry)> {
    let (cx, cy) = rect.center();
    outputs
        .iter()
        .find(|(_, geometry)| point_in_output(cx, cy, geometry))
        .or_else(|| outputs.first())
        .map(|(output, geometry)| (output, geometry))
}

fn point_in_output(x: f32, y: f32, output: &OutputGeometry) -> bool {
    x >= output.logical_x as f32
        && y >= output.logical_y as f32
        && x < (output.logical_x + output.logical_width) as f32
        && y < (output.logical_y + output.logical_height) as f32
}

fn output_geometry(info: &OutputInfo) -> OutputGeometry {
    let (logical_x, logical_y) = info.logical_position.unwrap_or(info.location);
    let (logical_width, logical_height) = info.logical_size.unwrap_or_else(|| {
        info.modes
            .iter()
            .find(|mode| mode.current)
            .map(|mode| mode.dimensions)
            .unwrap_or((1920, 1080))
    });
    OutputGeometry {
        logical_x,
        logical_y,
        logical_width: logical_width.max(1),
        logical_height: logical_height.max(1),
    }
}

fn draw_preview_spec(canvas: &mut [u8], width: u32, height: u32, spec: &PreviewSpec) {
    clear_canvas(canvas);
    match spec {
        PreviewSpec::Marker(spec) => draw_marker(canvas, width, height, spec),
        PreviewSpec::FixedJoystick(spec) => draw_fixed_joystick(canvas, width, height, spec),
        PreviewSpec::CornerGuide(spec) => draw_bitmap_corner_guide(canvas, width, height, spec),
        PreviewSpec::Dot(spec) => draw_dot(canvas, width, height, spec),
    }
}

fn clear_canvas(canvas: &mut [u8]) {
    for chunk in canvas.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[0, 0, 0, 0]);
    }
}

fn draw_marker(canvas: &mut [u8], width: u32, height: u32, spec: &MarkerSpec) {
    let center = (width as f32 * 0.5, height as f32 * 0.5);
    draw_circle_fill(canvas, width, height, center, spec.radius - 1.6, spec.fill);
    draw_circle_stroke(canvas, width, height, center, spec.radius, 1.8, spec.stroke);
    draw_text_centered(
        canvas,
        width,
        height,
        center,
        &spec.label,
        Rgba {
            r: 244,
            g: 245,
            b: 247,
            a: 255,
        },
    );
}

fn draw_fixed_joystick(canvas: &mut [u8], width: u32, height: u32, spec: &FixedJoystickSpec) {
    let center = (width as f32 * 0.5, height as f32 * 0.5);
    draw_circle_stroke(canvas, width, height, center, spec.radius, 1.8, spec.color);
    draw_circle_stroke(
        canvas,
        width,
        height,
        center,
        (spec.radius * 0.22).max(10.0),
        1.4,
        dim_color(spec.color, 0.82),
    );

    let label_radius = spec.radius + 18.0;
    draw_small_bubble(
        canvas,
        width,
        height,
        (center.0, center.1 - label_radius),
        &spec.keys[0],
        spec.color,
    );
    draw_small_bubble(
        canvas,
        width,
        height,
        (center.0 - label_radius, center.1),
        &spec.keys[1],
        spec.color,
    );
    draw_small_bubble(
        canvas,
        width,
        height,
        (center.0 + label_radius, center.1),
        &spec.keys[2],
        spec.color,
    );
    draw_small_bubble(
        canvas,
        width,
        height,
        (center.0, center.1 + label_radius),
        &spec.keys[3],
        spec.color,
    );
}

fn draw_small_bubble(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    center: (f32, f32),
    label: &str,
    color: Rgba,
) {
    draw_circle_fill(
        canvas,
        width,
        height,
        center,
        10.5,
        Rgba {
            r: color.r,
            g: color.g,
            b: color.b,
            a: 28,
        },
    );
    draw_circle_stroke(canvas, width, height, center, 11.0, 1.4, color);
    draw_text_centered(
        canvas,
        width,
        height,
        center,
        label,
        Rgba {
            r: 242,
            g: 243,
            b: 246,
            a: 255,
        },
    );
}

fn draw_bitmap_corner_guide(canvas: &mut [u8], width: u32, height: u32, spec: &CornerGuideSpec) {
    let stroke = spec.color;
    match spec.corner {
        GuideCorner::TopLeft => {
            draw_line(
                canvas,
                width,
                height,
                (1.0, 1.0),
                (GUIDE_SIZE as f32 - 2.0, 1.0),
                1.8,
                stroke,
            );
            draw_line(
                canvas,
                width,
                height,
                (1.0, 1.0),
                (1.0, GUIDE_SIZE as f32 - 2.0),
                1.8,
                stroke,
            );
        }
        GuideCorner::TopRight => {
            draw_line(
                canvas,
                width,
                height,
                (width as f32 - 2.0, 1.0),
                (2.0, 1.0),
                1.8,
                stroke,
            );
            draw_line(
                canvas,
                width,
                height,
                (width as f32 - 2.0, 1.0),
                (width as f32 - 2.0, GUIDE_SIZE as f32 - 2.0),
                1.8,
                stroke,
            );
        }
        GuideCorner::BottomLeft => {
            draw_line(
                canvas,
                width,
                height,
                (1.0, height as f32 - 2.0),
                (GUIDE_SIZE as f32 - 2.0, height as f32 - 2.0),
                1.8,
                stroke,
            );
            draw_line(
                canvas,
                width,
                height,
                (1.0, height as f32 - 2.0),
                (1.0, 2.0),
                1.8,
                stroke,
            );
        }
        GuideCorner::BottomRight => {
            draw_line(
                canvas,
                width,
                height,
                (width as f32 - 2.0, height as f32 - 2.0),
                (2.0, height as f32 - 2.0),
                1.8,
                stroke,
            );
            draw_line(
                canvas,
                width,
                height,
                (width as f32 - 2.0, height as f32 - 2.0),
                (width as f32 - 2.0, 2.0),
                1.8,
                stroke,
            );
        }
    }
}

fn draw_dot(canvas: &mut [u8], width: u32, height: u32, spec: &DotSpec) {
    let center = (width as f32 * 0.5, height as f32 * 0.5);
    draw_circle_fill(canvas, width, height, center, spec.radius, spec.color);
}

fn draw_circle_fill(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    center: (f32, f32),
    radius: f32,
    color: Rgba,
) {
    let min_x = ((center.0 - radius - 2.0).floor().max(0.0)) as u32;
    let max_x = ((center.0 + radius + 2.0).ceil().min(width as f32)) as u32;
    let min_y = ((center.1 - radius - 2.0).floor().max(0.0)) as u32;
    let max_y = ((center.1 + radius + 2.0).ceil().min(height as f32)) as u32;

    for y in min_y..max_y {
        for x in min_x..max_x {
            let coverage = sample_distance_coverage((x as f32, y as f32), |sx, sy| {
                let dx = sx - center.0;
                let dy = sy - center.1;
                radius - (dx * dx + dy * dy).sqrt()
            });
            if coverage > 0.0 {
                blend_pixel(canvas, width, x, y, multiply_alpha(color, coverage));
            }
        }
    }
}

fn draw_circle_stroke(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    center: (f32, f32),
    radius: f32,
    thickness: f32,
    color: Rgba,
) {
    let outer = radius + thickness * 0.5;
    let inner = (radius - thickness * 0.5).max(0.0);
    let min_x = ((center.0 - outer - 2.0).floor().max(0.0)) as u32;
    let max_x = ((center.0 + outer + 2.0).ceil().min(width as f32)) as u32;
    let min_y = ((center.1 - outer - 2.0).floor().max(0.0)) as u32;
    let max_y = ((center.1 + outer + 2.0).ceil().min(height as f32)) as u32;

    for y in min_y..max_y {
        for x in min_x..max_x {
            let coverage = sample_distance_coverage((x as f32, y as f32), |sx, sy| {
                let dx = sx - center.0;
                let dy = sy - center.1;
                let distance = (dx * dx + dy * dy).sqrt();
                let outer_cov = outer - distance;
                let inner_cov = distance - inner;
                outer_cov.min(inner_cov)
            });
            if coverage > 0.0 {
                blend_pixel(canvas, width, x, y, multiply_alpha(color, coverage));
            }
        }
    }
}

fn draw_line(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    start: (f32, f32),
    end: (f32, f32),
    thickness: f32,
    color: Rgba,
) {
    let min_x = (start.0.min(end.0) - thickness - 2.0).floor().max(0.0) as u32;
    let max_x = (start.0.max(end.0) + thickness + 2.0)
        .ceil()
        .min(width as f32) as u32;
    let min_y = (start.1.min(end.1) - thickness - 2.0).floor().max(0.0) as u32;
    let max_y = (start.1.max(end.1) + thickness + 2.0)
        .ceil()
        .min(height as f32) as u32;

    for y in min_y..max_y {
        for x in min_x..max_x {
            let coverage = sample_distance_coverage((x as f32, y as f32), |sx, sy| {
                thickness * 0.5 - distance_to_segment((sx, sy), start, end)
            });
            if coverage > 0.0 {
                blend_pixel(canvas, width, x, y, multiply_alpha(color, coverage));
            }
        }
    }
}

fn sample_distance_coverage(origin: (f32, f32), distance_fn: impl Fn(f32, f32) -> f32) -> f32 {
    let mut inside = 0u32;
    for sample_y in [0.125f32, 0.375, 0.625, 0.875] {
        for sample_x in [0.125f32, 0.375, 0.625, 0.875] {
            if distance_fn(origin.0 + sample_x, origin.1 + sample_y) >= 0.0 {
                inside += 1;
            }
        }
    }
    inside as f32 / 16.0
}

fn draw_text_centered(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    center: (f32, f32),
    text: &str,
    color: Rgba,
) {
    let text = display_label(text);
    let upper = text.to_uppercase();
    let char_count = upper.chars().count() as i32;
    let scale = if char_count <= 2 { 2 } else { 1 };
    let glyph_w = 6 * scale;
    let glyph_h = 7 * scale;
    let total_w = glyph_w * char_count;
    let start_x = center.0.round() as i32 - total_w / 2;
    let start_y = center.1.round() as i32 - glyph_h / 2;

    for (index, ch) in upper.chars().enumerate() {
        let Some(glyph) = glyph_rows(ch) else {
            continue;
        };
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << col) == 0 {
                    continue;
                }
                for sy in 0..scale {
                    for sx in 0..scale {
                        let x = start_x + index as i32 * glyph_w + col * scale + sx;
                        let y = start_y + row as i32 * scale + sy;
                        if x >= 0 && y >= 0 && (x as u32) < width && (y as u32) < height {
                            blend_pixel(canvas, width, x as u32, y as u32, color);
                        }
                    }
                }
            }
        }
    }
}

fn glyph_rows(ch: char) -> Option<[u8; 7]> {
    match ch {
        'A' => Some([
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ]),
        'B' => Some([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ]),
        'C' => Some([
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ]),
        'D' => Some([
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ]),
        'E' => Some([
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ]),
        'F' => Some([
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ]),
        'G' => Some([
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ]),
        'H' => Some([
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ]),
        'I' => Some([
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ]),
        'J' => Some([
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ]),
        'K' => Some([
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ]),
        'L' => Some([
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ]),
        'M' => Some([
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ]),
        'N' => Some([
            0b10001, 0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001,
        ]),
        'O' => Some([
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ]),
        'P' => Some([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ]),
        'Q' => Some([
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ]),
        'R' => Some([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ]),
        'S' => Some([
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ]),
        'T' => Some([
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ]),
        'U' => Some([
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ]),
        'V' => Some([
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ]),
        'W' => Some([
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ]),
        'X' => Some([
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ]),
        'Y' => Some([
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ]),
        'Z' => Some([
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ]),
        '0' => Some([
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ]),
        '1' => Some([
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ]),
        '2' => Some([
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ]),
        '3' => Some([
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ]),
        '4' => Some([
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ]),
        '5' => Some([
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ]),
        '6' => Some([
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ]),
        '7' => Some([
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ]),
        '8' => Some([
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ]),
        '9' => Some([
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ]),
        '+' => Some([
            0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
        ]),
        '-' => Some([
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ]),
        _ => None,
    }
}

fn blend_pixel(canvas: &mut [u8], width: u32, x: u32, y: u32, src: Rgba) {
    let idx = ((y * width + x) * 4) as usize;
    let dst = Rgba {
        b: canvas[idx],
        g: canvas[idx + 1],
        r: canvas[idx + 2],
        a: canvas[idx + 3],
    };

    let src_a = src.a as f32 / 255.0;
    let dst_a = dst.a as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= 0.0 {
        return;
    }

    let out_r = ((src.r as f32 * src_a + dst.r as f32 * dst_a * (1.0 - src_a)) / out_a).round();
    let out_g = ((src.g as f32 * src_a + dst.g as f32 * dst_a * (1.0 - src_a)) / out_a).round();
    let out_b = ((src.b as f32 * src_a + dst.b as f32 * dst_a * (1.0 - src_a)) / out_a).round();

    canvas[idx] = out_b.clamp(0.0, 255.0) as u8;
    canvas[idx + 1] = out_g.clamp(0.0, 255.0) as u8;
    canvas[idx + 2] = out_r.clamp(0.0, 255.0) as u8;
    canvas[idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

fn multiply_alpha(color: Rgba, factor: f32) -> Rgba {
    Rgba {
        a: (color.a as f32 * factor).round().clamp(0.0, 255.0) as u8,
        ..color
    }
}

fn distance_to_segment(point: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let ab = (b.0 - a.0, b.1 - a.1);
    let ap = (point.0 - a.0, point.1 - a.1);
    let len_sq = ab.0 * ab.0 + ab.1 * ab.1;
    if len_sq <= f32::EPSILON {
        return ((point.0 - a.0).powi(2) + (point.1 - a.1).powi(2)).sqrt();
    }
    let t = ((ap.0 * ab.0 + ap.1 * ab.1) / len_sq).clamp(0.0, 1.0);
    let closest = (a.0 + ab.0 * t, a.1 + ab.1 * t);
    ((point.0 - closest.0).powi(2) + (point.1 - closest.1).powi(2)).sqrt()
}

fn lerp(start: f32, end: f32, t: f32) -> f32 {
    start + (end - start) * t
}

fn marker_color(node: &Node) -> Rgba {
    match node {
        Node::Tap { .. } => rgba(66, 133, 244, 230),
        Node::HoldTap { .. } => rgba(234, 67, 53, 230),
        Node::ToggleTap { .. } => rgba(0, 172, 193, 230),
        Node::RepeatTap { .. } => rgba(171, 71, 188, 230),
        _ => rgba(255, 255, 255, 230),
    }
}

fn joystick_color() -> Rgba {
    rgba(114, 227, 146, 220)
}

fn drag_color() -> Rgba {
    rgba(0, 200, 140, 220)
}

fn wheel_color() -> Rgba {
    rgba(87, 175, 255, 220)
}

fn aim_color() -> Rgba {
    rgba(251, 188, 4, 220)
}

fn dim_color(color: Rgba, amount: f32) -> Rgba {
    Rgba {
        r: (color.r as f32 * amount).round() as u8,
        g: (color.g as f32 * amount).round() as u8,
        b: (color.b as f32 * amount).round() as u8,
        a: color.a,
    }
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba { r, g, b, a }
}

fn aim_label(mode: MouseCameraActivationMode) -> String {
    match mode {
        MouseCameraActivationMode::AlwaysOn => "Aim".into(),
        MouseCameraActivationMode::WhileHeld => "Hold".into(),
        MouseCameraActivationMode::Toggle => "Tgl".into(),
    }
}

fn display_label(label: &str) -> String {
    match label {
        "MouseLeft" => "LMB".into(),
        "MouseRight" => "RMB".into(),
        "MouseMiddle" => "MMB".into(),
        "LeftShift" => "Shift".into(),
        "RightShift" => "Shift".into(),
        "LeftCtrl" => "Ctrl".into(),
        "RightCtrl" => "Ctrl".into(),
        "LeftAlt" => "Alt".into(),
        "RightAlt" => "Alt".into(),
        "PageUp" => "PgUp".into(),
        "PageDown" => "PgDn".into(),
        other if other.chars().count() <= 6 => other.into(),
        other => other.chars().take(6).collect(),
    }
}

fn run_legacy_overlay(profile: Profile) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Phantom Overlay Preview (Experimental)")
            .with_app_id("phantom-overlay")
            .with_fullscreen(true)
            .with_always_on_top()
            .with_transparent(false)
            .with_decorations(false)
            .with_resizable(false)
            .with_mouse_passthrough(false),
        ..Default::default()
    };

    eframe::run_native(
        "phantom-overlay",
        options,
        Box::new(|cc| Ok(Box::new(OverlayApp::new(cc, profile)))),
    )
}

struct OverlayApp {
    profile: Profile,
}

impl OverlayApp {
    fn new(cc: &eframe::CreationContext<'_>, profile: Profile) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        Self { profile }
    }
}

impl eframe::App for OverlayApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(OVERLAY_BG))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let painter = ui.painter_at(rect);
                draw_profile_overlay(&painter, rect, &self.profile);
                draw_overlay_header(&painter, rect);
            });
    }
}

fn draw_profile_overlay(painter: &egui::Painter, rect: Rect, profile: &Profile) {
    for node in &profile.nodes {
        match node {
            Node::Tap { pos, .. }
            | Node::HoldTap { pos, .. }
            | Node::ToggleTap { pos, .. }
            | Node::RepeatTap { pos, .. } => draw_button_marker(painter, rect, node, pos),
            Node::Wheel {
                up_pos, down_pos, ..
            } => draw_wheel_markers(painter, rect, up_pos, down_pos),
            Node::Joystick {
                mode: JoystickMode::Fixed,
                pos,
                radius,
                ..
            } => draw_fixed_joystick_legacy(painter, rect, node, pos, *radius),
            Node::Joystick {
                mode: JoystickMode::Floating,
                region: Some(region),
                ..
            } => draw_floating_joystick(painter, rect, node, region),
            Node::Drag { start, end, .. } => draw_drag_gesture(painter, rect, node, start, end),
            Node::MouseCamera {
                anchor,
                activation_mode,
                ..
            } => draw_aim_marker(painter, rect, anchor, activation_mode),
            Node::Macro { .. } | Node::LayerShift { .. } => {}
            Node::Joystick {
                mode: JoystickMode::Floating,
                region: None,
                ..
            } => {}
        }
    }
}

fn draw_button_marker(painter: &egui::Painter, rect: Rect, node: &Node, pos: &RelPos) {
    let center = to_canvas_pos(rect, pos);
    draw_ring_marker(
        painter,
        center,
        MARKER_RADIUS,
        marker_stroke(node),
        compact_label(node),
    );
}

fn draw_wheel_markers(painter: &egui::Painter, rect: Rect, up_pos: &RelPos, down_pos: &RelPos) {
    let up_center = to_canvas_pos(rect, up_pos);
    let down_center = to_canvas_pos(rect, down_pos);
    let stroke = wheel_stroke();
    draw_ring_marker(
        painter,
        up_center,
        MARKER_RADIUS,
        Stroke::new(2.0, stroke),
        "Up",
    );
    draw_ring_marker(
        painter,
        down_center,
        MARKER_RADIUS,
        Stroke::new(2.0, stroke),
        "Dn",
    );
    painter.line_segment(
        [up_center, down_center],
        Stroke::new(1.5, stroke.gamma_multiply(0.7)),
    );
}

fn draw_fixed_joystick_legacy(
    painter: &egui::Painter,
    rect: Rect,
    node: &Node,
    pos: &RelPos,
    radius: f64,
) {
    let center = to_canvas_pos(rect, pos);
    let radius_px = rect.width().min(rect.height()) * radius as f32;
    painter.circle_stroke(center, radius_px, Stroke::new(2.0, joystick_stroke()));
    painter.circle_stroke(
        center,
        (radius_px * 0.22).max(10.0),
        Stroke::new(1.5, joystick_stroke().gamma_multiply(0.8)),
    );
    if let Node::Joystick { keys, .. } = node {
        let label_radius = radius_px + 22.0;
        draw_ring_marker(
            painter,
            center + Vec2::new(0.0, -label_radius),
            SMALL_MARKER_RADIUS,
            Stroke::new(1.5, joystick_stroke()),
            &keys.up,
        );
        draw_ring_marker(
            painter,
            center + Vec2::new(-label_radius, 0.0),
            SMALL_MARKER_RADIUS,
            Stroke::new(1.5, joystick_stroke()),
            &keys.left,
        );
        draw_ring_marker(
            painter,
            center + Vec2::new(label_radius, 0.0),
            SMALL_MARKER_RADIUS,
            Stroke::new(1.5, joystick_stroke()),
            &keys.right,
        );
        draw_ring_marker(
            painter,
            center + Vec2::new(0.0, label_radius),
            SMALL_MARKER_RADIUS,
            Stroke::new(1.5, joystick_stroke()),
            &keys.down,
        );
    }
}

fn draw_floating_joystick(painter: &egui::Painter, rect: Rect, node: &Node, region: &Region) {
    let zone = region_rect(rect, region);
    draw_corner_guides(painter, zone, joystick_stroke());
    draw_ring_marker(
        painter,
        zone.center(),
        SMALL_MARKER_RADIUS,
        Stroke::new(1.5, joystick_stroke()),
        compact_label(node),
    );
}

fn draw_drag_gesture(
    painter: &egui::Painter,
    rect: Rect,
    node: &Node,
    start: &RelPos,
    end: &RelPos,
) {
    let start = to_canvas_pos(rect, start);
    let end = to_canvas_pos(rect, end);
    painter.arrow(start, end - start, Stroke::new(2.0, drag_stroke()));
    draw_ring_marker(
        painter,
        start,
        SMALL_MARKER_RADIUS,
        Stroke::new(1.5, drag_stroke()),
        compact_label(node),
    );
    painter.circle_stroke(
        end,
        8.0,
        Stroke::new(1.5, drag_stroke().gamma_multiply(0.85)),
    );
}

fn draw_aim_marker(
    painter: &egui::Painter,
    rect: Rect,
    anchor: &RelPos,
    activation_mode: &MouseCameraActivationMode,
) {
    let center = to_canvas_pos(rect, anchor);
    let mode = match activation_mode {
        MouseCameraActivationMode::AlwaysOn => "Aim",
        MouseCameraActivationMode::WhileHeld => "Hold Aim",
        MouseCameraActivationMode::Toggle => "Toggle Aim",
    };
    draw_ring_marker(
        painter,
        center,
        MARKER_RADIUS,
        Stroke::new(2.0, look_stroke()),
        mode,
    );
}

fn draw_ring_marker(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    stroke: Stroke,
    text: impl AsRef<str>,
) {
    painter.circle_stroke(center, radius, stroke);
    painter.circle_filled(center, radius - 2.0, marker_fill(stroke.color));
    draw_centered_text(painter, center, text.as_ref(), Color32::WHITE);
}

fn draw_overlay_header(painter: &egui::Painter, rect: Rect) {
    let header_rect = Rect::from_min_size(
        rect.left_top() + Vec2::new(20.0, 20.0),
        Vec2::new(360.0, 30.0),
    );
    painter.rect_filled(header_rect, 8.0, Color32::from_black_alpha(190));
    painter.text(
        header_rect.center(),
        Align2::CENTER_CENTER,
        HEADER_TEXT,
        FontId::proportional(13.0),
        Color32::from_rgb(255, 226, 150),
    );
}

fn draw_centered_text(painter: &egui::Painter, center: Pos2, text: &str, color: Color32) {
    let font_size = if text.len() >= 8 {
        11.0
    } else if text.len() >= 5 {
        12.0
    } else {
        14.0
    };
    let font = FontId::proportional(font_size);
    painter.text(
        center + Vec2::new(1.0, 1.0),
        Align2::CENTER_CENTER,
        text,
        font.clone(),
        TEXT_SHADOW,
    );
    painter.text(center, Align2::CENTER_CENTER, text, font, color);
}

fn draw_corner_guides(painter: &egui::Painter, rect: Rect, color: Color32) {
    let stroke = Stroke::new(2.0, color);
    draw_corner_guide(
        painter,
        rect.left_top(),
        Vec2::new(GUIDE_LENGTH, 0.0),
        Vec2::new(0.0, GUIDE_LENGTH),
        stroke,
    );
    draw_corner_guide(
        painter,
        rect.right_top(),
        Vec2::new(-GUIDE_LENGTH, 0.0),
        Vec2::new(0.0, GUIDE_LENGTH),
        stroke,
    );
    draw_corner_guide(
        painter,
        rect.left_bottom(),
        Vec2::new(GUIDE_LENGTH, 0.0),
        Vec2::new(0.0, -GUIDE_LENGTH),
        stroke,
    );
    draw_corner_guide(
        painter,
        rect.right_bottom(),
        Vec2::new(-GUIDE_LENGTH, 0.0),
        Vec2::new(0.0, -GUIDE_LENGTH),
        stroke,
    );
}

fn draw_corner_guide(
    painter: &egui::Painter,
    corner: Pos2,
    horizontal: Vec2,
    vertical: Vec2,
    stroke: Stroke,
) {
    painter.line_segment([corner, corner + horizontal], stroke);
    painter.line_segment([corner, corner + vertical], stroke);
}

fn compact_label(node: &Node) -> String {
    match node {
        Node::Tap { key, .. }
        | Node::HoldTap { key, .. }
        | Node::ToggleTap { key, .. }
        | Node::Drag { key, .. }
        | Node::RepeatTap { key, .. } => key.clone(),
        Node::Wheel { .. } => "Wheel".into(),
        Node::Joystick { .. } => "Move".into(),
        Node::MouseCamera {
            activation_mode,
            activation_key,
            ..
        } => match activation_mode {
            MouseCameraActivationMode::AlwaysOn => "Aim".into(),
            MouseCameraActivationMode::WhileHeld => {
                activation_key.as_deref().unwrap_or("Aim").into()
            }
            MouseCameraActivationMode::Toggle => activation_key.as_deref().unwrap_or("Aim").into(),
        },
        Node::Macro { id, .. } | Node::LayerShift { id, .. } => id.clone(),
    }
}

fn to_canvas_pos(rect: Rect, pos: &RelPos) -> Pos2 {
    Pos2::new(
        rect.left() + rect.width() * pos.x as f32,
        rect.top() + rect.height() * pos.y as f32,
    )
}

fn region_rect(content: Rect, region: &Region) -> Rect {
    Rect::from_min_size(
        to_canvas_pos(
            content,
            &RelPos {
                x: region.x,
                y: region.y,
            },
        ),
        Vec2::new(
            content.width() * region.w as f32,
            content.height() * region.h as f32,
        ),
    )
}

fn marker_fill(color: Color32) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 18)
}

fn joystick_stroke() -> Color32 {
    Color32::from_rgba_unmultiplied(114, 227, 146, 210)
}

fn drag_stroke() -> Color32 {
    Color32::from_rgba_unmultiplied(0, 200, 140, 220)
}

fn wheel_stroke() -> Color32 {
    Color32::from_rgba_unmultiplied(87, 175, 255, 220)
}

fn look_stroke() -> Color32 {
    Color32::from_rgba_unmultiplied(251, 188, 4, 200)
}

fn marker_stroke(node: &Node) -> Stroke {
    let color = match node {
        Node::Tap { .. } => Color32::from_rgb(66, 133, 244),
        Node::HoldTap { .. } => Color32::from_rgb(234, 67, 53),
        Node::ToggleTap { .. } => Color32::from_rgb(0, 172, 193),
        Node::RepeatTap { .. } => Color32::from_rgb(171, 71, 188),
        _ => Color32::WHITE,
    };
    Stroke::new(2.0, color)
}
