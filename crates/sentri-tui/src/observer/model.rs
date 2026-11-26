//! The Observer as a 3D point model.
//!
//! An original ward design: a round head holding a lens-shaped eye on both
//! faces, two swept, feathered wings, a crest, and a staff that carries a
//! floating energy core down to a weighted base. Every surface is sampled
//! once into points with outward normals; the renderer rotates, projects and
//! shades them. Units: the head has radius 1, `y` points up and the front
//! of the ward faces `-z` (towards the viewer at rest).

use std::f32::consts::{PI, TAU};
use std::sync::OnceLock;

use super::Part;

/// A point on the surface of the ward.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub p: [f32; 3],
    /// Outward unit normal.
    pub n: [f32; 3],
    pub part: Part,
}

/// Centre of the head (and of the eye).
pub const HEAD: [f32; 3] = [0.0, 1.0, 0.0];
pub const HEAD_RADIUS: f32 = 1.0;
/// Lowest and highest points of the body, for fitting it on screen.
pub const BOTTOM: f32 = -2.95;
pub const TOP: f32 = 2.5;
/// Half the wingspan.
pub const SPAN: f32 = 3.2;

/// Surface sampling step; small enough to leave no holes on large screens.
const STEP: f32 = 0.03;

pub(crate) fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = v[0].mul_add(v[0], v[1].mul_add(v[1], v[2] * v[2])).sqrt();
    if len <= f32::EPSILON {
        [0.0, 0.0, -1.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1].mul_add(b[2], -(a[2] * b[1])),
        a[2].mul_add(b[0], -(a[0] * b[2])),
        a[0].mul_add(b[1], -(a[1] * b[0])),
    ]
}

/// How many steps cover `length` at the sampling density.
fn steps(length: f32) -> usize {
    // Lengths are small positive numbers, so the cast is exact enough.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let n = (length / STEP).ceil().max(1.0) as usize;
    n
}

#[allow(clippy::cast_precision_loss)]
fn frac(i: usize, n: usize) -> f32 {
    i as f32 / n as f32
}

/// The sphere of the head, with the lens slightly bulging on both faces.
fn head(out: &mut Vec<Sample>) {
    let rings = steps(PI * HEAD_RADIUS);
    for i in 0..=rings {
        let theta = PI * frac(i, rings);
        let (st, ct) = theta.sin_cos();
        let around = steps(TAU * HEAD_RADIUS * st.max(0.05));
        for j in 0..around {
            let phi = TAU * frac(j, around);
            let (sp, cp) = phi.sin_cos();
            let n = [st * cp, ct, st * sp];
            // The lens stands a little proud of the shell.
            let r = if n[2].abs() > super::EYE_EDGE {
                HEAD_RADIUS + 0.07
            } else {
                HEAD_RADIUS
            };
            out.push(Sample {
                p: [HEAD[0] + r * n[0], HEAD[1] + r * n[1], HEAD[2] + r * n[2]],
                n,
                part: Part::Shell,
            });
        }
    }
}

/// A swept wing on `side` (-1 left, 1 right) with a feathered lower edge.
fn wing(out: &mut Vec<Sample>, side: f32) {
    // Spine of the wing as `u` goes from root (0) to tip (1).
    let spine = |u: f32| -> [f32; 3] {
        [
            side * 2.35f32.mul_add(u, 0.8),
            HEAD[1] + 0.95 * u.powf(1.6) - 0.1,
            // The tips curl forward, cupping the eye.
            -0.55 * u * u + 0.1,
        ]
    };
    let top = |u: f32| 0.62f32.mul_add((1.0 - u).powf(0.7), 0.05);
    let bottom = |u: f32| {
        let feathers = 0.3f32.mul_add((u * 3.0 * PI).sin().abs(), 0.7);
        0.85 * (1.0 - u).powf(0.9) * feathers + 0.04
    };
    let along = steps(2.6);
    for i in 0..=along {
        let u = frac(i, along);
        let c = spine(u);
        let du = {
            let a = spine((u - 0.01).max(0.0));
            let b = spine((u + 0.01).min(1.0));
            [b[0] - a[0], b[1] - a[1], b[2] - a[2]]
        };
        let face = normalize(cross(du, [0.0, 1.0, 0.0]));
        // Point the face normal to the front.
        let front = if face[2] > 0.0 {
            [-face[0], -face[1], -face[2]]
        } else {
            face
        };
        let (lo, hi) = (-bottom(u), top(u));
        let across = steps(hi - lo);
        for j in 0..=across {
            let v = (hi - lo).mul_add(frac(j, across), lo);
            for normal in [front, [-front[0], -front[1], -front[2]]] {
                // Each face sits half the wing's thickness off the spine.
                out.push(Sample {
                    p: [
                        normal[0].mul_add(0.045, c[0]),
                        normal[1].mul_add(0.045, c[1] + v),
                        normal[2].mul_add(0.045, c[2]),
                    ],
                    n: normal,
                    part: Part::Wing,
                });
            }
        }
    }
}

/// A thin blade on top of the head, swept back.
fn crest(out: &mut Vec<Sample>) {
    let along = steps(0.9);
    for i in 0..=along {
        let u = frac(i, along);
        let height = 0.55 * (1.0 - u);
        let base = [0.0, HEAD[1] + 0.92, 0.35f32.mul_add(u, -0.25)];
        let up = steps(height.max(STEP));
        for j in 0..=up {
            let y = base[1] + height * frac(j, up);
            for nx in [-1.0, 1.0] {
                out.push(Sample {
                    p: [0.03 * nx, y, base[2] + 0.25 * frac(j, up)],
                    n: [nx, 0.0, 0.0],
                    part: Part::Wing,
                });
            }
        }
    }
}

/// A surface of revolution around the `y` axis: `radius(y)` from `y0` up
/// to `y1`.
fn lathe(out: &mut Vec<Sample>, y0: f32, y1: f32, radius: impl Fn(f32) -> f32, part: Part) {
    let up = steps(y1 - y0);
    for i in 0..=up {
        let y = (y1 - y0).mul_add(frac(i, up), y0);
        let r = radius(y);
        let slope = (radius(y + 0.01) - radius(y - 0.01)) / 0.02;
        let around = steps(TAU * r.max(0.05));
        for j in 0..around {
            let phi = TAU * frac(j, around);
            let (sp, cp) = phi.sin_cos();
            out.push(Sample {
                p: [r * cp, y, r * sp],
                n: normalize([cp, -slope, sp]),
                part,
            });
        }
    }
}

/// Every sample of the ward, computed once.
pub fn ward() -> &'static [Sample] {
    static WARD: OnceLock<Vec<Sample>> = OnceLock::new();
    WARD.get_or_init(|| {
        let mut out = Vec::with_capacity(40_000);
        head(&mut out);
        wing(&mut out, -1.0);
        wing(&mut out, 1.0);
        crest(&mut out);
        // Collar joining the head to the staff.
        lathe(
            &mut out,
            -0.2,
            0.15,
            |y| 0.18 + (y + 0.2) * 1.0,
            Part::Staff,
        );
        // The staff.
        lathe(&mut out, -2.55, -0.2, |_| 0.14, Part::Staff);
        // The energy core: a double cone floating on the staff.
        lathe(
            &mut out,
            -1.55,
            -0.75,
            |y| 0.42 * (1.0 - ((y + 1.15) / 0.4).abs()) + 0.15,
            Part::Core,
        );
        // Weighted base.
        lathe(
            &mut out,
            BOTTOM,
            -2.55,
            |y| 0.25 + 0.65 * ((-2.55 - y) / 0.4),
            Part::Base,
        );
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ward_is_symmetric_and_bounded() {
        let ward = ward();
        assert!(ward.len() > 10_000, "dense enough: {}", ward.len());
        let mut left = 0usize;
        let mut right = 0usize;
        for s in ward {
            let [x, y, _] = s.p;
            assert!(x.abs() <= SPAN + 0.1, "{s:?}");
            assert!((BOTTOM - 0.01..=TOP + 0.01).contains(&y), "{s:?}");
            let len = s.n.iter().map(|c| c * c).sum::<f32>().sqrt();
            assert!((len - 1.0).abs() < 1e-3, "unit normal {s:?}");
            if s.part == Part::Wing && x < -0.5 {
                left += 1;
            } else if s.part == Part::Wing && x > 0.5 {
                right += 1;
            }
        }
        assert_eq!(left, right, "both wings match");
        for part in [Part::Shell, Part::Wing, Part::Staff, Part::Core, Part::Base] {
            assert!(ward.iter().any(|s| s.part == part), "{part:?}");
        }
    }
}
