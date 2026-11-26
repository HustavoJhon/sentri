//! Turns the 3D ward into terminal characters.
//!
//! Each frame rotates the sampled model around its vertical axis, tilts it
//! slightly towards the viewer, projects it with a gentle perspective onto a
//! grid of cells (which are about twice as tall as they are wide) and keeps
//! the nearest sample per cell. Brightness comes from a fixed light and maps
//! to characters of increasing density, so the shape reads as solid and
//! stays stable from frame to frame: nothing is randomized.

// Rendering converts between small grid sizes, tick counts and floats all
// the time; every value involved is far below the precision limits.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::f32::consts::{PI, TAU};

use super::model::{self, Sample};
use super::{EYE_EDGE, Eye, Part};

/// Characters from darkest to brightest.
const RAMP: &[u8] = b".,:;=+*#%";
/// Iris characters from dim to bright; kept apart from the body ramp so
/// the eye always stands out.
const IRIS: [char; 3] = ['o', 'O', '0'];
/// Terminal cells are about twice as tall as they are wide.
const ASPECT: f32 = 2.0;
/// Camera tilt: the ward is seen slightly from above.
const PITCH: f32 = 0.24;
/// Distance from the camera to the ward's centre.
const DISTANCE: f32 = 11.0;
/// Model-space extent that has to fit on screen.
const FIT_TOP: f32 = model::TOP + 0.15;
const FIT_BOTTOM: f32 = model::BOTTOM - 0.85;
const FIT_HALF_WIDTH: f32 = model::SPAN + 0.25;
/// Height of the energy core the rings orbit.
const CORE: f32 = -1.15;
/// Ground radar radius.
const RADAR: f32 = 2.75;

/// What to draw this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scene {
    /// Rotation around the vertical axis, in radians.
    pub yaw: f32,
    /// Overall illumination, 0 (dark) to 1.
    pub light: f32,
    pub eye: Eye,
    /// Animation tick, for the pulse, rings, sweep and particles.
    pub tick: u64,
    /// Orbiting rings to show (0, 1 or 2).
    pub rings: u8,
    pub radar: bool,
    /// Ambient particles to scatter around the ward.
    pub particles: u8,
    pub unicode: bool,
}

/// One drawn cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub part: Part,
    /// Brightness level from 0 (faint) to 3 (brightest), for coloring.
    pub level: u8,
}

/// A rendered frame: `height` rows of `width` cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Option<Cell>>,
}

impl Canvas {
    pub fn get(&self, x: usize, y: usize) -> Option<Cell> {
        self.cells.get(y * self.width + x).copied().flatten()
    }

    /// The frame as plain text, one line per row (for tests and previews).
    pub fn text(&self) -> String {
        (0..self.height)
            .map(|y| {
                (0..self.width)
                    .map(|x| self.get(x, y).map_or(' ', |c| c.ch))
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}

/// Rotation around `y` by `yaw`, then around `x` by the camera pitch.
#[derive(Debug, Clone, Copy)]
struct View {
    sy: f32,
    cy: f32,
    sp: f32,
    cp: f32,
}

impl View {
    fn new(yaw: f32) -> Self {
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = PITCH.sin_cos();
        Self { sy, cy, sp, cp }
    }

    /// Model space to camera space (direction only).
    fn turn(self, v: [f32; 3]) -> [f32; 3] {
        let x = self.cy.mul_add(v[0], self.sy * v[2]);
        let z = (-self.sy).mul_add(v[0], self.cy * v[2]);
        self.tilt([x, v[1], z])
    }

    /// World space (already turned) to camera space.
    fn tilt(self, v: [f32; 3]) -> [f32; 3] {
        let y = self.cp.mul_add(v[1], self.sp * v[2]);
        let z = (-self.sp).mul_add(v[1], self.cp * v[2]);
        [v[0], y, z]
    }
}

/// Projection of camera space onto the grid.
#[derive(Debug, Clone, Copy)]
struct Lens {
    kx: f32,
    ky: f32,
    cx: f32,
    cy: f32,
    mid: f32,
}

impl Lens {
    fn new(width: usize, height: usize) -> Self {
        let (w, h) = (width as f32, height as f32);
        let tall = FIT_TOP - FIT_BOTTOM;
        let mut ky = (h - 0.5) / tall;
        let mut kx = ky * ASPECT;
        if kx * 2.0 * FIT_HALF_WIDTH > w - 1.0 {
            kx = (w - 1.0) / (2.0 * FIT_HALF_WIDTH);
            ky = kx / ASPECT;
        }
        Self {
            kx,
            ky,
            cx: w / 2.0,
            cy: h / 2.0,
            mid: f32::midpoint(FIT_TOP, FIT_BOTTOM),
        }
    }

    /// Grid cell and depth for a camera-space point.
    fn project(self, p: [f32; 3]) -> (i32, i32, f32) {
        let depth = DISTANCE + p[2];
        let f = DISTANCE / depth;
        let x = p[0].mul_add(f * self.kx, self.cx).floor() as i32;
        let y = (-(p[1] - self.mid)).mul_add(f * self.ky, self.cy).floor() as i32;
        (x, y, depth)
    }
}

/// Direction towards the light, in camera space (above, left, in front).
fn light_dir() -> [f32; 3] {
    model::normalize([-0.45, 0.62, -0.64])
}

/// Brightness of a surface with camera-space normal `n`, from 0 to 1.
fn shade(n: [f32; 3], light: f32) -> f32 {
    let l = light_dir();
    let diffuse = dot(n, l).max(0.0);
    // Blinn highlight towards a viewer looking along +z.
    let half = model::normalize([l[0], l[1], l[2] - 1.0]);
    let spec = dot(n, half).max(0.0).powi(16);
    (0.62f32.mul_add(diffuse, 0.14) + 0.35 * spec).min(1.0) * light
}
fn ramp(lum: f32) -> (char, u8) {
    let lum = lum.clamp(0.0, 1.0);
    let i = (lum * (RAMP.len() - 1) as f32).round() as usize;
    let level = (lum * 3.99) as u8;
    (char::from(RAMP[i.min(RAMP.len() - 1)]), level)
}
fn iris(lum: f32) -> char {
    IRIS[((lum.clamp(0.0, 1.0) * 2.99) as usize).min(2)]
}

/// How the eye region of the head is drawn, from the model normal `n`.
fn eye_cell(n: [f32; 3], lum: f32, scene: &Scene) -> Option<Cell> {
    let z = n[2].abs();
    if z <= EYE_EDGE - 0.09 {
        return None;
    }
    if z <= EYE_EDGE {
        // A dark socket outlines the lens.
        return Some(Cell {
            ch: if scene.unicode { '·' } else { '.' },
            part: Part::Base,
            level: 0,
        });
    }
    // The pupil watches left and right while awake.
    let look = (scene.tick as f32 * 0.05).sin() * 0.28;
    let facing = n[2].signum();
    let pupil_dir = model::normalize([look * -facing, -0.05, facing]);
    let pupil = dot(n, pupil_dir) > 0.965;
    let slit = n[1].abs();
    let lid = |ch: char, level: u8| {
        Some(Cell {
            ch,
            part: Part::Eye,
            level,
        })
    };
    match scene.eye {
        Eye::Closed if slit < 0.07 => lid('-', 0),
        Eye::Stirring if slit < 0.13 => {
            if pupil {
                lid('o', 1)
            } else {
                lid('=', 1)
            }
        }
        Eye::Half if n[1] < 0.12 => {
            if pupil {
                lid(if scene.unicode { '●' } else { 'o' }, 2)
            } else {
                lid(iris(lum), 2)
            }
        }
        Eye::Open => {
            if pupil {
                Some(Cell {
                    ch: if scene.unicode { '◉' } else { '@' },
                    part: Part::Pupil,
                    level: 3,
                })
            } else {
                // A slow pulse: the iris breathes between two levels.
                let pulse = (scene.tick as f32 * 0.21).sin();
                lid(iris(lum + 0.15 * pulse), if pulse > 0.0 { 3 } else { 2 })
            }
        }
        // The lid: shaded like the shell.
        _ => None,
    }
}

struct Raster {
    lens: Lens,
    width: usize,
    height: usize,
    depth: Vec<f32>,
    cells: Vec<Option<Cell>>,
}

impl Raster {
    fn plot(&mut self, p: [f32; 3], cell: Cell) {
        let (x, y, depth) = self.lens.project(p);
        let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
            return;
        };
        if x >= self.width || y >= self.height {
            return;
        }
        let i = y * self.width + x;
        if depth < self.depth[i] {
            self.depth[i] = depth;
            self.cells[i] = Some(cell);
        }
    }
}

fn body(r: &mut Raster, view: View, scene: &Scene) {
    let model_light = scene.light;
    for &Sample { p, n, part } in model::ward() {
        let nc = view.turn(n);
        // Surfaces turned away are hidden by the ones in front.
        if nc[2] > 0.3 {
            continue;
        }
        let lum = shade(nc, model_light);
        let cell = if part == Part::Shell {
            eye_cell(n, lum, scene)
        } else {
            None
        };
        let cell = cell.unwrap_or_else(|| {
            let boost = if part == Part::Core && scene.eye >= Eye::Half {
                // The core glows with the eye.
                0.25f32.mul_add((scene.tick as f32 * 0.21).sin(), 0.3)
            } else {
                0.0
            };
            let (ch, level) = ramp(lum + boost);
            Cell { ch, part, level }
        });
        r.plot(view.turn(p), cell);
    }
}

/// Dashed energy rings orbiting the core, turning in opposite directions
/// like a gyroscope.
fn rings(r: &mut Raster, view: View, scene: &Scene) {
    let t = scene.tick as f32;
    let specs: [(f32, f32, f32, f32); 2] = [
        // radius, tilt around x, tilt around z, spin per tick
        (1.2, 0.12, 0.1, 0.07),
        (0.95, -0.3, -0.35, -0.1),
    ];
    for &(radius, ax, az, spin) in specs.iter().take(scene.rings.into()) {
        let (sx, cx) = ax.sin_cos();
        let (sz, cz) = az.sin_cos();
        let points = 200;
        for i in 0..points {
            let a = TAU * i as f32 / points as f32;
            // Three dashes per ring, moving with the spin.
            let dash = (a * 3.0 + t * spin * 3.0).rem_euclid(TAU);
            if dash > PI * 1.2 {
                continue;
            }
            let (sa, ca) = a.sin_cos();
            // Circle in the xz plane, then tilted.
            let p = [radius * ca, 0.0, radius * sa];
            let p = [
                p[0],
                (-sx).mul_add(p[2], cx * p[1]),
                sx.mul_add(p[1], cx * p[2]),
            ];
            let p = [
                cz.mul_add(p[0], -(sz * p[1])),
                sz.mul_add(p[0], cz * p[1]),
                p[2],
            ];
            let cam = view.tilt([p[0], p[1] + CORE, p[2]]);
            let near = cam[2] < 0.0;
            let head = dash < 0.4;
            let (ch, level) = match (near, head, scene.unicode) {
                (true, true, true) => ('•', 3),
                (true, true, false) => ('o', 3),
                (true, false, _) => ('-', 2),
                (false, _, true) => ('·', 1),
                (false, _, false) => ('.', 1),
            };
            r.plot(
                cam,
                Cell {
                    ch,
                    part: Part::Ring,
                    level,
                },
            );
        }
    }
}

/// The scanner on the ground: a dotted circle with a sweeping beam.
fn radar(r: &mut Raster, view: View, scene: &Scene) {
    let sweep = (scene.tick as f32 * 0.1).rem_euclid(TAU);
    let ground = model::BOTTOM - 0.02;
    let points: u32 = 300;
    for i in 0..points {
        let a = TAU * i as f32 / points as f32;
        let (sa, ca) = a.sin_cos();
        let behind = (sweep - a).rem_euclid(TAU);
        let cam = view.tilt([RADAR * ca, ground, RADAR * sa]);
        let cell = if behind < 1.4 {
            let level = 3 - (behind / 0.47) as u8;
            let ch = match level {
                3 => '=',
                2 => '-',
                _ => {
                    if scene.unicode {
                        '·'
                    } else {
                        '.'
                    }
                }
            };
            Cell {
                ch,
                part: Part::Radar,
                level,
            }
        } else if i.is_multiple_of(3) {
            Cell {
                ch: if scene.unicode { '·' } else { '.' },
                part: Part::Radar,
                level: 0,
            }
        } else {
            continue;
        };
        r.plot(cam, cell);
    }
    // The beam from the base out to the rim.
    let (sa, ca) = sweep.sin_cos();
    for i in 0..60 {
        let d = (RADAR - 1.0).mul_add(i as f32 / 59.0, 1.0);
        let cam = view.tilt([d * ca, ground, d * sa]);
        let ch = if scene.unicode { '•' } else { '*' };
        let level = if i > 45 { 3 } else { 2 };
        r.plot(
            cam,
            Cell {
                ch,
                part: Part::Radar,
                level,
            },
        );
    }
}

/// A small, stable hash for deterministic particle placement.
const fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// Motes rising slowly through the empty space around the ward.
fn particles(raster: &mut Raster, scene: &Scene) {
    let (cols, rows) = (raster.width as u64, raster.height as u64);
    if cols < 8 || rows < 6 {
        return;
    }
    for mote in 0..u64::from(scene.particles) {
        let hash = mix(mote);
        let column = hash % cols;
        let period = rows + 6;
        let pace = 3 + (hash >> 8) % 4; // ticks per row
        let age = (scene.tick / pace + (hash >> 16) % period) % period;
        let Some(row) = (rows + 2).checked_sub(age) else {
            continue;
        };
        if row >= rows {
            continue;
        }
        let sway = (((scene.tick + (hash >> 24) % 50) as f32) * 0.07).sin() * 1.4;
        let col = (column as f32 + sway).round().clamp(0.0, (cols - 1) as f32) as usize;
        let index = row as usize * raster.width + col;
        if raster.cells[index].is_some() {
            continue;
        }
        // Brightest mid-flight, fading near the top.
        let life = row as f32 / rows as f32;
        let level = if life > 0.25 && life < 0.75 { 2 } else { 1 };
        let star = (hash >> 40).is_multiple_of(5) && level == 2;
        let ch = match (star, scene.unicode) {
            (true, true) => '✦',
            (true, false) => '*',
            (false, true) => '·',
            (false, false) => '.',
        };
        raster.cells[index] = Some(Cell {
            ch,
            part: Part::Particle,
            level,
        });
    }
}

/// Renders `scene` onto a `width` x `height` grid.
pub fn render(scene: &Scene, width: usize, height: usize) -> Canvas {
    let mut r = Raster {
        lens: Lens::new(width, height),
        width,
        height,
        depth: vec![f32::INFINITY; width * height],
        cells: vec![None; width * height],
    };
    if width == 0 || height == 0 {
        return Canvas {
            width,
            height,
            cells: r.cells,
        };
    }
    let view = View::new(scene.yaw);
    if scene.radar {
        radar(&mut r, view, scene);
    }
    body(&mut r, view, scene);
    rings(&mut r, view, scene);
    particles(&mut r, scene);
    Canvas {
        width,
        height,
        cells: r.cells,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(yaw: f32, eye: Eye) -> Scene {
        Scene {
            yaw,
            light: 1.0,
            eye,
            tick: 7,
            rings: 2,
            radar: true,
            particles: 8,
            unicode: false,
        }
    }

    fn filled(c: &Canvas) -> usize {
        c.cells.iter().filter(|c| c.is_some()).count()
    }

    #[test]
    fn frames_are_deterministic_and_fit_the_grid() {
        for (w, h) in [(56, 24), (40, 16), (90, 36), (3, 2)] {
            let a = render(&scene(0.4, Eye::Open), w, h);
            let b = render(&scene(0.4, Eye::Open), w, h);
            assert_eq!(a, b, "same scene, same frame");
            assert_eq!(a.cells.len(), w * h);
        }
    }

    #[test]
    fn the_ward_stays_recognizable_all_the_way_round() {
        let front = render(&scene(0.0, Eye::Open), 56, 24);
        let front_cells = filled(&front);
        for step in 0..24 {
            let yaw = TAU * step as f32 / 24.0;
            let c = render(&scene(yaw, Eye::Open), 56, 24);
            let n = filled(&c);
            // The silhouette never collapses or explodes while turning.
            assert!(n * 3 > front_cells, "yaw {yaw}: {n} vs {front_cells}");
            assert!(n < front_cells * 3, "yaw {yaw}: {n} vs {front_cells}");
            // The staff is always on the vertical axis below the head.
            let mid = c.width / 2;
            let staff = (0..c.height).any(|y| {
                (mid - 1..=mid + 1).any(|x| c.get(x, y).is_some_and(|c| c.part == Part::Staff))
            });
            assert!(staff, "yaw {yaw}\n{}", c.text());
        }
    }

    #[test]
    fn the_eye_faces_the_viewer_front_and_back() {
        let pupil = |yaw: f32| {
            let c = render(&scene(yaw, Eye::Open), 56, 24);
            c.cells
                .iter()
                .flatten()
                .filter(|c| c.part == Part::Pupil)
                .count()
        };
        let (front, back, side) = (pupil(0.0), pupil(PI), pupil(PI / 2.0));
        assert!(front > 0 && back > 0, "front {front}, back {back}");
        // Seen from the side the lens is edge-on.
        assert!(side * 3 < front, "side {side} vs front {front}");
    }

    #[test]
    fn a_closed_eye_has_no_pupil_and_darkness_dims_everything() {
        let closed = render(&scene(0.0, Eye::Closed), 56, 24);
        assert!(!closed.cells.iter().flatten().any(|c| c.part == Part::Pupil));
        let dark = Scene {
            light: 0.2,
            ..scene(0.0, Eye::Closed)
        };
        let dim = render(&dark, 56, 24);
        let bright = |c: &Canvas| {
            c.cells
                .iter()
                .flatten()
                .filter(|c| c.part == Part::Shell && c.level >= 2)
                .count()
        };
        assert!(bright(&dim) < bright(&closed));
    }

    #[test]
    fn ascii_scenes_use_only_ascii() {
        let c = render(&scene(1.0, Eye::Open), 56, 24);
        assert!(c.text().is_ascii());
    }
}
