use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
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

use phantom::overlay::CursorOverlayState;

const CURSOR_SURFACE_SIZE: u32 = 40;
const CURSOR_HOTSPOT_X: i32 = 5;
const CURSOR_HOTSPOT_Y: i32 = 4;

pub fn run_cursor_overlay(state_path: &Path) -> Result<()> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("WAYLAND_SOCKET").is_some()
    {
        run_wayland_cursor_overlay(state_path)
    } else {
        Err(anyhow::anyhow!(
            "cursor overlay is only implemented for Wayland layer-shell sessions"
        ))
    }
}

fn run_wayland_cursor_overlay(state_path: &Path) -> Result<()> {
    let conn = Connection::connect_to_env().context("failed to connect to Wayland compositor")?;
    let (globals, mut event_queue) =
        registry_queue_init(&conn).context("failed to initialize Wayland registry")?;
    let qh = event_queue.handle();

    let mut app = CursorLayerApp {
        registry_state: RegistryState::new(&globals),
        compositor_state: CompositorState::bind(&globals, &qh)
            .context("wl_compositor not available")?,
        output_state: OutputState::new(&globals, &qh),
        shm: Shm::bind(&globals, &qh).context("wl_shm not available")?,
        layer_shell: LayerShell::bind(&globals, &qh).context("layer shell not available")?,
        state_path: state_path.to_path_buf(),
        last_state: read_cursor_overlay_state(state_path).unwrap_or(CursorOverlayState {
            visible: false,
            pressed: false,
            screen_x: 0.0,
            screen_y: 0.0,
        }),
        overlays: Vec::new(),
        exit: false,
    };

    event_queue
        .roundtrip(&mut app)
        .context("failed to collect initial Wayland globals")?;
    app.create_overlays(&qh)?;
    event_queue
        .roundtrip(&mut app)
        .context("failed to configure cursor overlay surfaces")?;

    while !app.exit {
        event_queue
            .blocking_dispatch(&mut app)
            .context("cursor overlay Wayland dispatch failed")?;
    }

    Ok(())
}

#[derive(Clone)]
struct OutputGeometry {
    logical_x: i32,
    logical_y: i32,
    logical_width: i32,
    logical_height: i32,
}

struct OutputOverlay {
    output: wl_output::WlOutput,
    geometry: OutputGeometry,
    layer: LayerSurface,
    pool: SlotPool,
    width: u32,
    height: u32,
    configured: bool,
}

struct CursorLayerApp {
    registry_state: RegistryState,
    compositor_state: CompositorState,
    output_state: OutputState,
    shm: Shm,
    layer_shell: LayerShell,
    state_path: PathBuf,
    last_state: CursorOverlayState,
    overlays: Vec<OutputOverlay>,
    exit: bool,
}

impl CursorLayerApp {
    fn create_overlays(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        self.overlays.clear();

        let outputs: Vec<_> = self.output_state.outputs().collect();
        for output in outputs {
            let Some(info) = self.output_state.info(&output) else {
                continue;
            };
            let geometry = output_geometry(&info);
            let surface = self.compositor_state.create_surface(qh);
            let layer = self.layer_shell.create_layer_surface(
                qh,
                surface,
                Layer::Overlay,
                Some("phantom-cursor-overlay"),
                Some(&output),
            );
            layer.set_anchor(Anchor::TOP | Anchor::LEFT);
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            layer.set_exclusive_zone(-1);
            layer.set_size(CURSOR_SURFACE_SIZE, CURSOR_SURFACE_SIZE);
            layer.set_margin(0, 0, 0, 0);

            let region = self.compositor_state.wl_compositor().create_region(qh, ());
            layer.set_input_region(Some(&region));
            region.destroy();
            layer.commit();

            let pool = SlotPool::new(
                (CURSOR_SURFACE_SIZE * CURSOR_SURFACE_SIZE * 4) as usize,
                &self.shm,
            )
            .context("failed to create cursor overlay shm pool")?;

            self.overlays.push(OutputOverlay {
                output,
                geometry,
                layer,
                pool,
                width: CURSOR_SURFACE_SIZE,
                height: CURSOR_SURFACE_SIZE,
                configured: false,
            });
        }

        if self.overlays.is_empty() {
            return Err(anyhow::anyhow!(
                "no Wayland outputs available for cursor overlay"
            ));
        }

        Ok(())
    }

    fn update_state(&mut self) {
        if let Some(state) = read_cursor_overlay_state(&self.state_path) {
            self.last_state = state;
        }
    }

    fn draw_overlay(&mut self, qh: &QueueHandle<Self>, index: usize) {
        self.update_state();
        let overlay = match self.overlays.get_mut(index) {
            Some(overlay) => overlay,
            None => return,
        };
        if !overlay.configured {
            return;
        }

        let stride = overlay.width as i32 * 4;
        let (buffer, canvas) = match overlay.pool.create_buffer(
            overlay.width as i32,
            overlay.height as i32,
            stride,
            wl_shm::Format::Argb8888,
        ) {
            Ok(buffer) => buffer,
            Err(e) => {
                tracing::warn!("cursor overlay buffer allocation failed: {}", e);
                return;
            }
        };

        let local_x = self.last_state.screen_x as i32 - overlay.geometry.logical_x;
        let local_y = self.last_state.screen_y as i32 - overlay.geometry.logical_y;
        let on_output = self.last_state.visible
            && local_x >= 0
            && local_y >= 0
            && local_x < overlay.geometry.logical_width
            && local_y < overlay.geometry.logical_height;

        if on_output {
            overlay.layer.set_margin(
                (local_y - CURSOR_HOTSPOT_Y).max(0),
                0,
                0,
                (local_x - CURSOR_HOTSPOT_X).max(0),
            );
        }

        draw_canvas(
            canvas,
            overlay.width,
            overlay.height,
            on_output,
            self.last_state.pressed,
        );

        overlay
            .layer
            .wl_surface()
            .damage_buffer(0, 0, overlay.width as i32, overlay.height as i32);
        overlay
            .layer
            .wl_surface()
            .frame(qh, overlay.layer.wl_surface().clone());
        buffer
            .attach_to(overlay.layer.wl_surface())
            .expect("cursor overlay buffer attach");
        overlay.layer.commit();
    }

    fn overlay_index_for_surface(&self, surface: &wl_surface::WlSurface) -> Option<usize> {
        self.overlays
            .iter()
            .position(|overlay| overlay.layer.wl_surface() == surface)
    }
}

impl CompositorHandler for CursorLayerApp {
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
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        if let Some(index) = self.overlay_index_for_surface(surface) {
            self.draw_overlay(qh, index);
        }
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

impl LayerShellHandler for CursorLayerApp {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if let Some(index) = self
            .overlays
            .iter()
            .position(|overlay| overlay.layer == *layer)
        {
            let overlay = &mut self.overlays[index];
            if configure.new_size.0 != 0 && configure.new_size.1 != 0 {
                overlay.width = configure.new_size.0.max(CURSOR_SURFACE_SIZE);
                overlay.height = configure.new_size.1.max(CURSOR_SURFACE_SIZE);
            }
            overlay.configured = true;
            self.draw_overlay(qh, index);
        }
    }
}

impl OutputHandler for CursorLayerApp {
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
        output: wl_output::WlOutput,
    ) {
        if let Some(info) = self.output_state.info(&output) {
            if let Some(overlay) = self
                .overlays
                .iter_mut()
                .find(|overlay| overlay.output == output)
            {
                overlay.geometry = output_geometry(&info);
            }
        }
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.overlays.retain(|overlay| overlay.output != output);
        if self.overlays.is_empty() {
            self.exit = true;
        }
    }
}

impl ShmHandler for CursorLayerApp {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_compositor!(CursorLayerApp);
delegate_output!(CursorLayerApp);
delegate_shm!(CursorLayerApp);
delegate_layer!(CursorLayerApp);
delegate_registry!(CursorLayerApp);

impl ProvidesRegistryState for CursorLayerApp {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState];
}

impl Dispatch<wl_region::WlRegion, ()> for CursorLayerApp {
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

fn draw_canvas(canvas: &mut [u8], width: u32, height: u32, visible: bool, pressed: bool) {
    for chunk in canvas.chunks_exact_mut(4) {
        chunk.copy_from_slice(&0u32.to_le_bytes());
    }
    if !visible {
        return;
    }

    let arrow = [
        (5.0f32, 4.0f32),
        (5.0, 30.0),
        (10.6, 24.7),
        (14.2, 35.0),
        (16.9, 33.8),
        (13.4, 23.9),
        (22.4, 23.9),
    ];
    let shadow_offset = (2.4f32, 1.9f32);
    let shadow_arrow = translated_polygon(&arrow, shadow_offset.0, shadow_offset.1);
    let outline = [34u8, 34u8, 38u8, 255u8];
    let fill = if pressed {
        [236u8, 238u8, 242u8, 255u8]
    } else {
        [248u8, 248u8, 250u8, 255u8]
    };

    for y in 0..height {
        for x in 0..width {
            let point = (x as f32, y as f32);
            let shadow_cov = sample_polygon_coverage(point, &shadow_arrow);
            let fill_cov = sample_polygon_coverage(point, &arrow);
            let outline_cov = sample_outline_coverage(point, &arrow, 1.05);

            let index = ((y * width + x) * 4) as usize;
            if shadow_cov > 0.0 {
                let alpha = (shadow_cov * 74.0).round() as u8;
                canvas[index..index + 4].copy_from_slice(&[0, 0, 0, alpha]);
            }
            if fill_cov > 0.0 {
                let alpha = (fill_cov * fill[3] as f32).round() as u8;
                canvas[index..index + 4].copy_from_slice(&[fill[0], fill[1], fill[2], alpha]);
            }
            if outline_cov > 0.0 {
                let alpha = (outline_cov * outline[3] as f32).round() as u8;
                canvas[index..index + 4]
                    .copy_from_slice(&[outline[0], outline[1], outline[2], alpha]);
            }
        }
    }
}

fn translated_polygon(polygon: &[(f32, f32)], dx: f32, dy: f32) -> Vec<(f32, f32)> {
    polygon.iter().map(|(x, y)| (x + dx, y + dy)).collect()
}

fn sample_polygon_coverage(origin: (f32, f32), polygon: &[(f32, f32)]) -> f32 {
    let mut inside = 0u32;
    for sample_y in [0.125f32, 0.375, 0.625, 0.875] {
        for sample_x in [0.125f32, 0.375, 0.625, 0.875] {
            if point_in_polygon((origin.0 + sample_x, origin.1 + sample_y), polygon) {
                inside += 1;
            }
        }
    }
    inside as f32 / 16.0
}

fn sample_outline_coverage(origin: (f32, f32), polygon: &[(f32, f32)], thickness: f32) -> f32 {
    let mut covered = 0u32;
    for sample_y in [0.125f32, 0.375, 0.625, 0.875] {
        for sample_x in [0.125f32, 0.375, 0.625, 0.875] {
            let point = (origin.0 + sample_x, origin.1 + sample_y);
            let distance = polygon_edge_distance(point, polygon);
            if point_in_polygon(point, polygon) && distance <= thickness {
                covered += 1;
            }
        }
    }
    covered as f32 / 16.0
}

fn point_in_polygon(point: (f32, f32), polygon: &[(f32, f32)]) -> bool {
    let (px, py) = point;
    let mut inside = false;
    let mut previous = *polygon.last().expect("cursor polygon");
    for &current in polygon {
        let (cx, cy) = current;
        let (px0, py0) = previous;
        let intersects =
            ((cy > py) != (py0 > py)) && (px < (px0 - cx) * (py - cy) / (py0 - cy) + cx);
        if intersects {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

fn polygon_edge_distance(point: (f32, f32), polygon: &[(f32, f32)]) -> f32 {
    let mut previous = *polygon.last().expect("cursor polygon");
    let mut best = f32::INFINITY;
    for &current in polygon {
        best = best.min(distance_to_segment(point, previous, current));
        previous = current;
    }
    best
}

fn distance_to_segment(point: (f32, f32), start: (f32, f32), end: (f32, f32)) -> f32 {
    let (px, py) = point;
    let (sx, sy) = start;
    let (ex, ey) = end;
    let vx = ex - sx;
    let vy = ey - sy;
    let length_sq = vx * vx + vy * vy;
    if length_sq <= f32::EPSILON {
        return ((px - sx).powi(2) + (py - sy).powi(2)).sqrt();
    }

    let t = (((px - sx) * vx) + ((py - sy) * vy)) / length_sq;
    let t = t.clamp(0.0, 1.0);
    let closest_x = sx + t * vx;
    let closest_y = sy + t * vy;
    ((px - closest_x).powi(2) + (py - closest_y).powi(2)).sqrt()
}

fn read_cursor_overlay_state(path: &Path) -> Option<CursorOverlayState> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}
