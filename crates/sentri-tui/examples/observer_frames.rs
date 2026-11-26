//! Prints Observer frames as plain text, for checking proportions.
//!
//! `cargo run -p sentri-tui --example observer_frames -- [width] [height] [frames]`
use sentri_tui::observer::Eye;
use sentri_tui::observer::render::{Scene, render};

fn main() {
    let args: Vec<usize> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let (w, h, n) = (
        args.first().copied().unwrap_or(56),
        args.get(1).copied().unwrap_or(24),
        args.get(2).copied().unwrap_or(8),
    );
    for i in 0..n {
        #[allow(clippy::cast_precision_loss)]
        let yaw = std::f32::consts::TAU * i as f32 / n as f32;
        let scene = Scene {
            yaw,
            light: 1.0,
            eye: Eye::Open,
            tick: i as u64 * 5,
            rings: 2,
            radar: true,
            particles: 8,
            unicode: false,
        };
        println!("--- yaw {:.0}°", yaw.to_degrees());
        println!("{}", render(&scene, w, h).text());
    }
}
