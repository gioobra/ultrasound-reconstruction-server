//! Conversão do vetor f para PNG em escala de cinza (`specs.md` §4.6).

use anyhow::Result;
use std::path::Path;

/// `f` é reorganizado em width×width em ordem coluna-major (hipótese H3):
/// pixel(linha, coluna) = f[coluna·width + linha]. Com `transpose`, linha e coluna trocam.
pub fn to_gray(f: &[f64], width: usize, scale: u32, transpose: bool) -> ::image::GrayImage {
    assert_eq!(f.len(), width * width);
    let (min, max) = f
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    let range = max - min;
    let w = width as u32;
    ::image::GrayImage::from_fn(w * scale, w * scale, |x, y| {
        let (lin, col) = ((y / scale) as usize, (x / scale) as usize);
        let (lin, col) = if transpose { (col, lin) } else { (lin, col) };
        let v = f[col * width + lin];
        let px = if range > 0.0 { 255.0 * (v - min) / range } else { 0.0 };
        ::image::Luma([px.round().clamp(0.0, 255.0) as u8])
    })
}

pub fn save_png(f: &[f64], width: usize, scale: u32, transpose: bool, path: &Path) -> Result<()> {
    to_gray(f, width, scale, transpose).save(path)?;
    Ok(())
}
