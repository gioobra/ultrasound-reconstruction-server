//! CGNE e CGNR (`specs.md` §4). Versão de referência, um sinal por vez.

use crate::matrix::HMatrix;
use crate::ops::{dot, matvec, norm2, rmatvec};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Algorithm {
    #[serde(rename = "cgne")]
    Cgne,
    #[serde(rename = "cgnr")]
    Cgnr,
}

impl Algorithm {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cgne" => Some(Self::Cgne),
            "cgnr" => Some(Self::Cgnr),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub epsilon: f64,
    pub max_iterations: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self { epsilon: 1e-4, max_iterations: 10 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Converged,
    MaxIterations,
    Breakdown,
}

#[derive(Debug, Clone)]
pub struct Reconstruction {
    pub f: Vec<f64>,
    pub iterations: usize,
    pub stop_reason: StopReason,
    pub final_epsilon: f64,
    pub final_residual_norm: f64,
    /// λ = max(|Hᵀg|)·0.10
    pub lambda: f64,
    /// ε de cada iteração (para diagnóstico)
    pub epsilon_history: Vec<f64>,
    /// ‖r‖₂ ao final de cada iteração
    pub residual_history: Vec<f64>,
}

pub fn reconstruct(h: &HMatrix, g: &[f64], algo: Algorithm, cfg: Config) -> Reconstruction {
    match algo {
        Algorithm::Cgne => cgne(h, g, cfg),
        Algorithm::Cgnr => cgnr(h, g, cfg),
    }
}

fn lambda_of(htg: &[f64]) -> f64 {
    htg.iter().fold(0.0f64, |m, v| m.max(v.abs())) * 0.10
}

fn axpy(y: &mut [f64], a: f64, x: &[f64]) {
    for (yy, xx) in y.iter_mut().zip(x) {
        *yy += a * xx;
    }
}

pub fn cgne(h: &HMatrix, g: &[f64], cfg: Config) -> Reconstruction {
    let (rows, cols) = (h.rows(), h.cols());
    assert_eq!(g.len(), rows);
    let mut f = vec![0.0; cols];
    let mut r = g.to_vec(); // r0 = g − H·0
    let mut p = vec![0.0; cols];
    rmatvec(h, &r, &mut p); // p0 = Hᵀ r0
    let lambda = lambda_of(&p);
    let mut w = vec![0.0; rows];
    let mut rr = dot(&r, &r);

    let mut out = Reconstruction {
        f: Vec::new(),
        iterations: 0,
        stop_reason: StopReason::MaxIterations,
        final_epsilon: f64::NAN,
        final_residual_norm: rr.sqrt(),
        lambda,
        epsilon_history: vec![],
        residual_history: vec![],
    };

    for i in 0..cfg.max_iterations {
        let pp = dot(&p, &p);
        if rr == 0.0 {
            out.stop_reason = StopReason::Converged;
            break;
        }
        let alpha = rr / pp;
        if !alpha.is_finite() {
            out.stop_reason = StopReason::Breakdown;
            break;
        }
        axpy(&mut f, alpha, &p);
        matvec(h, &p, &mut w);
        axpy(&mut r, -alpha, &w);
        let rr_new = dot(&r, &r);
        let eps = (rr_new.sqrt() - rr.sqrt()).abs();
        out.iterations = i + 1;
        out.final_epsilon = eps;
        out.final_residual_norm = rr_new.sqrt();
        out.epsilon_history.push(eps);
        out.residual_history.push(rr_new.sqrt());
        if !eps.is_finite() {
            out.stop_reason = StopReason::Breakdown;
            break;
        }
        if eps < cfg.epsilon {
            out.stop_reason = StopReason::Converged;
            break;
        }
        let beta = rr_new / rr;
        let mut hr = vec![0.0; cols];
        rmatvec(h, &r, &mut hr);
        for (pp_, hh) in p.iter_mut().zip(&hr) {
            *pp_ = hh + beta * *pp_;
        }
        rr = rr_new;
    }
    out.f = f;
    out
}

pub fn cgnr(h: &HMatrix, g: &[f64], cfg: Config) -> Reconstruction {
    let (rows, cols) = (h.rows(), h.cols());
    assert_eq!(g.len(), rows);
    let mut f = vec![0.0; cols];
    let mut r = g.to_vec();
    let mut z = vec![0.0; cols];
    rmatvec(h, &r, &mut z); // z0 = Hᵀ r0
    let lambda = lambda_of(&z);
    let mut p = z.clone();
    let mut w = vec![0.0; rows];
    let mut zz = dot(&z, &z);

    let mut out = Reconstruction {
        f: Vec::new(),
        iterations: 0,
        stop_reason: StopReason::MaxIterations,
        final_epsilon: f64::NAN,
        final_residual_norm: norm2(&r),
        lambda,
        epsilon_history: vec![],
        residual_history: vec![],
    };

    for i in 0..cfg.max_iterations {
        if zz == 0.0 {
            out.stop_reason = StopReason::Converged;
            break;
        }
        matvec(h, &p, &mut w);
        let ww = dot(&w, &w);
        let alpha = zz / ww;
        if !alpha.is_finite() {
            out.stop_reason = StopReason::Breakdown;
            break;
        }
        let r_norm_old = norm2(&r);
        axpy(&mut f, alpha, &p);
        axpy(&mut r, -alpha, &w);
        let r_norm = norm2(&r);
        let eps = (r_norm - r_norm_old).abs();
        out.iterations = i + 1;
        out.final_epsilon = eps;
        out.final_residual_norm = r_norm;
        out.epsilon_history.push(eps);
        out.residual_history.push(r_norm);
        if !eps.is_finite() {
            out.stop_reason = StopReason::Breakdown;
            break;
        }
        if eps < cfg.epsilon {
            out.stop_reason = StopReason::Converged;
            break;
        }
        rmatvec(h, &r, &mut z);
        let zz_new = dot(&z, &z);
        let beta = zz_new / zz;
        for (pp_, zz_) in p.iter_mut().zip(&z) {
            *pp_ = zz_ + beta * *pp_;
        }
        zz = zz_new;
    }
    out.f = f;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::write_bin;

    /// H 6×4 de posto completo, coluna-major, escrita em arquivo temporário.
    fn small_h(name: &str) -> HMatrix {
        let rows = 6;
        let cols = 4;
        let row_major: [[f64; 4]; 6] = [
            [4.0, 1.0, 0.0, 2.0],
            [1.0, 3.0, 1.0, 0.0],
            [0.0, 1.0, 5.0, 1.0],
            [2.0, 0.0, 1.0, 6.0],
            [1.0, 1.0, 1.0, 1.0],
            [0.5, 2.0, 0.0, 1.0],
        ];
        let mut data = vec![0.0; rows * cols];
        for i in 0..rows {
            for j in 0..cols {
                data[j * rows + i] = row_major[i][j];
            }
        }
        let path = std::env::temp_dir().join(format!("aps-test-{name}-{}.bin", std::process::id()));
        write_bin(&path, rows, cols, &data).unwrap();
        let h = HMatrix::open(&path).unwrap();
        let _ = std::fs::remove_file(&path); // o mmap continua válido no Linux
        h
    }

    fn check(algo: Algorithm, name: &str) {
        let h = small_h(name);
        let f_true = [1.0, 2.0, 3.0, 4.0];
        let mut g = vec![0.0; h.rows()];
        matvec(&h, &f_true, &mut g);
        let cfg = Config { epsilon: 1e-14, max_iterations: 50 };
        let rec = reconstruct(&h, &g, algo, cfg);
        for (a, b) in rec.f.iter().zip(f_true) {
            assert!((a - b).abs() < 1e-8, "{algo:?}: {a} != {b}");
        }
    }

    #[test]
    fn cgne_resolve_sistema_pequeno() {
        check(Algorithm::Cgne, "cgne");
    }

    #[test]
    fn cgnr_resolve_sistema_pequeno() {
        check(Algorithm::Cgnr, "cgnr");
    }

    #[test]
    fn nunca_passa_do_maximo_de_iteracoes() {
        let h = small_h("max");
        let mut g = vec![0.0; h.rows()];
        matvec(&h, &[1.0, 2.0, 3.0, 4.0], &mut g);
        for algo in [Algorithm::Cgne, Algorithm::Cgnr] {
            let rec = reconstruct(&h, &g, algo, Config { epsilon: 0.0, max_iterations: 2 });
            assert!(rec.iterations <= 2);
        }
    }
}
