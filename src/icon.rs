//! Dibuja los gráficos de donut: el icono de la bandeja (dos anillos concéntricos,
//! sesión fuera y semanal dentro) y los iconos de cada línea del menú.

use std::f64::consts::TAU;

use ksni::Icon;

type Rgba = [u8; 4];

/// Gris semitransparente: se distingue tanto en paneles oscuros como claros.
const TRACK: Rgba = [140, 140, 140, 110];
const GREEN: Rgba = [74, 222, 128, 255];
const AMBER: Rgba = [251, 191, 36, 255];
const RED: Rgba = [248, 113, 113, 255];
/// Anillo sin datos.
const GREY_TRACK: Rgba = [160, 160, 160, 90];

/// Submuestras por eje para el antialiasing.
const SS: i32 = 4;

pub fn level_color(percent: f64) -> Rgba {
    match percent {
        p if p >= 90.0 => RED,
        p if p >= 70.0 => AMBER,
        _ => GREEN,
    }
}

/// Un anillo, con radios relativos al tamaño (0.5 = borde del icono).
struct Ring {
    outer: f64,
    inner: f64,
    /// `None` = dato desconocido.
    percent: Option<f64>,
}

/// Icono de la bandeja. `None` en un valor significa dato desconocido.
pub fn render(session: Option<f64>, weekly: Option<f64>) -> Vec<Icon> {
    let rings = [
        Ring {
            outer: 0.50,
            inner: 0.33,
            percent: session,
        },
        Ring {
            outer: 0.27,
            inner: 0.10,
            percent: weekly,
        },
    ];
    [22, 32, 48, 64]
        .into_iter()
        .map(|size| {
            let rgba = draw(size, &rings);
            Icon {
                width: size,
                height: size,
                // SNI espera ARGB32 en orden de bytes de red.
                data: rgba
                    .chunks_exact(4)
                    .flat_map(|p| [p[3], p[0], p[1], p[2]])
                    .collect(),
            }
        })
        .collect()
}

/// PNG de un donut sencillo para las líneas del menú.
pub fn menu_png(percent: f64) -> Vec<u8> {
    const SIZE: i32 = 32;
    let rgba = draw(
        SIZE,
        &[Ring {
            outer: 0.46,
            inner: 0.24,
            percent: Some(percent),
        }],
    );
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, SIZE as u32, SIZE as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    // Escribir en un Vec en memoria no puede fallar.
    let mut writer = enc.write_header().expect("cabecera PNG");
    writer.write_image_data(&rgba).expect("datos PNG");
    writer.finish().expect("fin PNG");
    out
}

/// Devuelve RGBA no premultiplicado, fila a fila.
fn draw(size: i32, rings: &[Ring]) -> Vec<u8> {
    let n = size as f64;
    let c = n / 2.0;
    let samples = (SS * SS) as f64;
    let mut out = Vec::with_capacity((size * size * 4) as usize);

    for py in 0..size {
        for px in 0..size {
            // Acumula color premultiplicado para mezclar bien los bordes.
            let mut acc = [0.0f64; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = px as f64 + (sx as f64 + 0.5) / SS as f64 - c;
                    let y = py as f64 + (sy as f64 + 0.5) / SS as f64 - c;
                    if let Some(col) = sample(x, y, n, rings) {
                        let a = col[3] as f64 / 255.0;
                        for i in 0..3 {
                            acc[i] += col[i] as f64 * a;
                        }
                        acc[3] += a;
                    }
                }
            }
            if acc[3] == 0.0 {
                out.extend([0, 0, 0, 0]);
            } else {
                out.extend([
                    (acc[0] / acc[3]).round() as u8,
                    (acc[1] / acc[3]).round() as u8,
                    (acc[2] / acc[3]).round() as u8,
                    (acc[3] / samples * 255.0).round() as u8,
                ]);
            }
        }
    }
    out
}

fn sample(x: f64, y: f64, size: f64, rings: &[Ring]) -> Option<Rgba> {
    let r = (x * x + y * y).sqrt() / size;
    let ring = rings.iter().find(|g| r <= g.outer && r >= g.inner)?;
    let Some(p) = ring.percent else {
        return Some(GREY_TRACK);
    };
    // Ángulo desde las 12 en punto, en sentido horario, como fracción de vuelta.
    let turn = (x.atan2(-y) + TAU) % TAU / TAU;
    let p = p.clamp(0.0, 100.0);
    Some(if turn * 100.0 < p {
        level_color(p)
    } else {
        TRACK
    })
}
