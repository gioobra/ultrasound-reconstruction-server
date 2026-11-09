//! Ganho de sinal (`specs.md` §4.5): γₗ = 100 + (1/20)·l·√l, l = 1..S, g[l,c] *= γₗ.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// idx = c·S + (l−1): as S amostras do sensor 0, depois as do sensor 1, … (hipótese H2)
    SensorMajor,
    /// idx = (l−1)·N + c: para cada amostra, os N sensores
    SampleMajor,
}

pub fn gamma(l: usize) -> f64 {
    let l = l as f64;
    100.0 + l * l.sqrt() / 20.0
}

pub fn apply(g: &mut [f64], s: usize, n: usize, layout: Layout) {
    assert_eq!(g.len(), s * n, "tamanho de g diferente de S·N");
    for c in 0..n {
        for l in 1..=s {
            let idx = match layout {
                Layout::SensorMajor => c * s + (l - 1),
                Layout::SampleMajor => (l - 1) * n + c,
            };
            g[idx] *= gamma(l);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ganho_em_vetor_de_uns_devolve_gamma() {
        let (s, n) = (5, 3);
        let mut g = vec![1.0; s * n];
        apply(&mut g, s, n, Layout::SensorMajor);
        for c in 0..n {
            for l in 1..=s {
                assert_eq!(g[c * s + l - 1], gamma(l));
            }
        }
        assert!((gamma(1) - 100.05).abs() < 1e-12);
    }
}
