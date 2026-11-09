//! Operações de álgebra linear sobre H (versão simples com rayon; a fase 5 otimiza).

use crate::matrix::HMatrix;
use rayon::prelude::*;

const ROW_CHUNK: usize = 4096;

pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    debug_assert_eq!(a.len(), b.len());
    let mut acc = [0.0f64; 4];
    let ca = a.chunks_exact(4);
    let cb = b.chunks_exact(4);
    let (ra, rb) = (ca.remainder(), cb.remainder());
    for (x, y) in ca.zip(cb) {
        for k in 0..4 {
            acc[k] += x[k] * y[k];
        }
    }
    let mut s = acc[0] + acc[1] + acc[2] + acc[3];
    for (x, y) in ra.iter().zip(rb) {
        s += x * y;
    }
    s
}

pub fn norm2(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

/// y = H·x   (x: cols, y: rows)
pub fn matvec(h: &HMatrix, x: &[f64], y: &mut [f64]) {
    assert_eq!(x.len(), h.cols());
    assert_eq!(y.len(), h.rows());
    y.par_chunks_mut(ROW_CHUNK).enumerate().for_each(|(ci, yc)| {
        let r0 = ci * ROW_CHUNK;
        yc.fill(0.0);
        for (j, &xj) in x.iter().enumerate() {
            if xj == 0.0 {
                continue;
            }
            let col = &h.col(j)[r0..r0 + yc.len()];
            for (yy, &a) in yc.iter_mut().zip(col) {
                *yy += a * xj;
            }
        }
    });
}

/// z = Hᵀ·r   (r: rows, z: cols)
pub fn rmatvec(h: &HMatrix, r: &[f64], z: &mut [f64]) {
    assert_eq!(r.len(), h.rows());
    assert_eq!(z.len(), h.cols());
    z.par_iter_mut()
        .enumerate()
        .for_each(|(j, zj)| *zj = dot(h.col(j), r));
}

/// c = ‖HᵀH‖₂ por iteração de potência (σ_max(H)²).
pub fn spectral_norm_hth(h: &HMatrix, max_iter: usize, rel_tol: f64) -> f64 {
    let (rows, cols) = (h.rows(), h.cols());
    let mut v: Vec<f64> = (0..cols)
        .map(|i| 0.1 + ((i as u64).wrapping_mul(2654435761) % 1000) as f64 / 1000.0)
        .collect();
    let n = norm2(&v);
    v.iter_mut().for_each(|x| *x /= n);
    let mut w = vec![0.0; rows];
    let mut z = vec![0.0; cols];
    let mut lambda = 0.0;
    for _ in 0..max_iter {
        matvec(h, &v, &mut w);
        rmatvec(h, &w, &mut z);
        let new_lambda = norm2(&z);
        if new_lambda == 0.0 {
            return 0.0;
        }
        v.iter_mut().zip(&z).for_each(|(a, b)| *a = b / new_lambda);
        let done = lambda > 0.0 && ((new_lambda - lambda) / new_lambda).abs() < rel_tol;
        lambda = new_lambda;
        if done {
            break;
        }
    }
    lambda
}
