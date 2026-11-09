//! Ferramentas da fase 0: preparar H (CSV → binário), reconstruir um sinal e validar o ganho.

use anyhow::{bail, Context, Result};
use aps_core::algo::{self, Algorithm, Config};
use aps_core::gain::{self, Layout};
use aps_core::matrix::{write_bin, HMatrix};
use aps_core::model::ModelId;
use aps_core::{image as img, ops};
use clap::{Parser, Subcommand};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Parser)]
#[command(name = "aps-tools", about = "Ferramentas de preparação e validação")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Converte H-*.csv.zip em cache/H-*.bin e calcula c = ‖HᵀH‖₂
    Prepare {
        #[arg(long, default_value = "data")]
        data: PathBuf,
        #[arg(long, default_value = "cache")]
        out: PathBuf,
        /// 60x60 ou 30x30; sem o argumento, prepara os dois
        #[arg(long)]
        model: Option<String>,
    },
    /// Reconstrói um sinal (sem servidor) e grava um PNG
    Reconstruct {
        #[arg(long, default_value = "cache")]
        cache: PathBuf,
        #[arg(long)]
        model: String,
        #[arg(long)]
        signal: PathBuf,
        #[arg(long, default_value = "cgnr")]
        algorithm: String,
        #[arg(long)]
        png: PathBuf,
        /// Aplica o ganho antes de reconstruir
        #[arg(long)]
        gain: bool,
        #[arg(long, default_value_t = 1e-4)]
        epsilon: f64,
        #[arg(long, default_value_t = 10)]
        max_iterations: usize,
        /// Troca linha/coluna ao montar a imagem (teste da hipótese H3)
        #[arg(long)]
        transpose: bool,
    },
    /// Compara (sinal sem ganho + ganho) com um sinal de referência (A-*.csv) nos dois layouts
    CheckGain {
        #[arg(long)]
        model: String,
        #[arg(long)]
        signal: PathBuf,
        #[arg(long)]
        reference: PathBuf,
    },
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Prepare { data, out, model } => prepare(&data, &out, model.as_deref()),
        Cmd::Reconstruct { cache, model, signal, algorithm, png, gain, epsilon, max_iterations, transpose } => {
            reconstruct(&cache, &model, &signal, &algorithm, &png, gain, epsilon, max_iterations, transpose)
        }
        Cmd::CheckGain { model, signal, reference } => check_gain(&model, &signal, &reference),
    }
}

fn parse_model(s: &str) -> Result<ModelId> {
    ModelId::parse(s).with_context(|| format!("modelo desconhecido: {s} (use 60x60 ou 30x30)"))
}

fn read_signal(path: &Path) -> Result<Vec<f64>> {
    let file = File::open(path).with_context(|| format!("abrindo {}", path.display()))?;
    let mut v = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        let t = line.trim();
        if !t.is_empty() {
            v.push(t.parse::<f64>().with_context(|| format!("valor inválido: {t}"))?);
        }
    }
    Ok(v)
}

fn prepare(data: &Path, out: &Path, model: Option<&str>) -> Result<()> {
    fs::create_dir_all(out)?;
    let models: Vec<ModelId> = match model {
        Some(m) => vec![parse_model(m)?],
        None => ModelId::ALL.to_vec(),
    };
    for m in models {
        let t0 = Instant::now();
        let zip_path = data.join(format!("{}.csv.zip", m.h_stem()));
        let bin_path = out.join(format!("{}.bin", m.h_stem()));
        let (rows, cols) = (m.rows(), m.cols());
        println!("[{}] lendo {} (esperado {rows}×{cols})", m.name(), zip_path.display());

        let mut archive = zip::ZipArchive::new(File::open(&zip_path).with_context(|| zip_path.display().to_string())?)?;
        let idx = (0..archive.len())
            .find(|&i| {
                let n = archive.by_index_raw(i).map(|f| f.name().to_string()).unwrap_or_default();
                n.ends_with(".csv") && !n.contains("__MACOSX")
            })
            .context("nenhum .csv dentro do zip")?;
        let entry = archive.by_index(idx)?;
        let reader = BufReader::with_capacity(1 << 20, entry);

        // CSV é linha-major; o binário é coluna-major.
        let mut col_major = vec![0.0f64; rows * cols];
        let mut nrows = 0usize;
        let mut nnz = 0usize;
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if nrows >= rows {
                bail!("CSV tem mais de {rows} linhas");
            }
            let mut ncols = 0usize;
            for (j, tok) in line.split(',').enumerate() {
                if j >= cols {
                    bail!("linha {} tem mais de {cols} colunas", nrows + 1);
                }
                let v: f64 = tok.trim().parse().with_context(|| format!("linha {} col {}: '{tok}'", nrows + 1, j + 1))?;
                if v != 0.0 {
                    nnz += 1;
                }
                col_major[j * rows + nrows] = v;
                ncols += 1;
            }
            if ncols != cols {
                bail!("linha {} tem {ncols} colunas, esperado {cols}", nrows + 1);
            }
            nrows += 1;
            if nrows % 10000 == 0 {
                println!("[{}]   {nrows}/{rows} linhas", m.name());
            }
        }
        if nrows != rows {
            bail!("CSV tem {nrows} linhas, esperado {rows}");
        }
        println!(
            "[{}] densidade: {:.2}% não-zeros; escrevendo {}",
            m.name(),
            100.0 * nnz as f64 / (rows * cols) as f64,
            bin_path.display()
        );
        write_bin(&bin_path, rows, cols, &col_major)?;
        drop(col_major);

        let h = HMatrix::open(&bin_path)?;
        println!("[{}] calculando c = ‖HᵀH‖₂ (iteração de potência)…", m.name());
        let c = ops::spectral_norm_hth(&h, 100, 1e-6);
        let json = serde_json::json!({ "model": m.name(), "rows": rows, "cols": cols, "c": c });
        fs::write(out.join(format!("c-{}.json", m.name())), serde_json::to_string_pretty(&json)?)?;
        println!("[{}] c = {c:.6e}  ({:.1}s)", m.name(), t0.elapsed().as_secs_f64());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn reconstruct(
    cache: &Path,
    model: &str,
    signal: &Path,
    algorithm: &str,
    png: &Path,
    gain_on: bool,
    epsilon: f64,
    max_iterations: usize,
    transpose: bool,
) -> Result<()> {
    let m = parse_model(model)?;
    let algo = Algorithm::parse(algorithm).context("algoritmo deve ser cgne ou cgnr")?;
    let h = HMatrix::open(&cache.join(format!("{}.bin", m.h_stem())))
        .context("rode `aps-tools prepare` antes")?;
    let mut g = read_signal(signal)?;
    if g.len() != h.rows() {
        bail!("sinal tem {} valores, esperado {}", g.len(), h.rows());
    }
    if gain_on {
        gain::apply(&mut g, m.s(), m.n(), Layout::SensorMajor);
    }
    let t0 = Instant::now();
    let rec = algo::reconstruct(&h, &g, algo, Config { epsilon, max_iterations });
    let dt = t0.elapsed();
    println!("‖g‖₂ = {:.6e}   ganho = {gain_on}", ops::norm2(&g));
    println!("algoritmo = {algorithm}   iterações = {}   parada = {:?}", rec.iterations, rec.stop_reason);
    for (i, (e, r)) in rec.epsilon_history.iter().zip(&rec.residual_history).enumerate() {
        println!("  iter {:>2}: ε = {e:.3e}   ‖r‖ = {r:.6e}", i + 1);
    }
    println!("λ = {:.6e}   tempo = {:.3}s", rec.lambda, dt.as_secs_f64());
    img::save_png(&rec.f, m.width(), 4, transpose, png)?;
    println!("PNG: {}", png.display());
    Ok(())
}

fn check_gain(model: &str, signal: &Path, reference: &Path) -> Result<()> {
    let m = parse_model(model)?;
    let base = read_signal(signal)?;
    let reference = read_signal(reference)?;
    if base.len() != m.rows() || reference.len() != m.rows() {
        bail!("tamanhos: sinal={}, referência={}, esperado={}", base.len(), reference.len(), m.rows());
    }
    let ref_max = reference.iter().fold(0.0f64, |a, v| a.max(v.abs()));
    for (name, layout) in [("sensor-major (H2)", Layout::SensorMajor), ("sample-major", Layout::SampleMajor)] {
        let mut g = base.clone();
        gain::apply(&mut g, m.s(), m.n(), layout);
        let max_err = g.iter().zip(&reference).fold(0.0f64, |a, (x, y)| a.max((x - y).abs()));
        let ratio: Vec<f64> = g.iter().zip(&reference).filter(|(_, y)| y.abs() > 0.0).map(|(x, y)| x / y).take(5).collect();
        println!("{name:<20} erro máx. relativo = {:.3e}   primeiras razões g/ref = {ratio:?}", max_err / ref_max);
    }
    Ok(())
}
