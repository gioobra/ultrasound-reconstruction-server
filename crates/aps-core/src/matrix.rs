//! Formato binário de H (`specs`/`arquitetura.md` §4.1) e acesso via mmap.
//!
//! Cabeçalho de 64 bytes: magic "APSH", version u32, rows u64, cols u64, dtype u32.
//! Dados em f64 little-endian, coluna-major.

use anyhow::{bail, ensure, Context, Result};
use memmap2::Mmap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

pub const MAGIC: &[u8; 4] = b"APSH";
pub const VERSION: u32 = 1;
pub const HEADER_LEN: usize = 64;
pub const DTYPE_F64: u32 = 0;

pub struct HMatrix {
    mmap: Mmap,
    rows: usize,
    cols: usize,
}

impl HMatrix {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("abrindo {}", path.display()))?;
        // SAFETY: o arquivo é tratado como somente leitura e não é modificado durante a execução.
        let mmap = unsafe { Mmap::map(&file)? };
        ensure!(mmap.len() >= HEADER_LEN, "arquivo menor que o cabeçalho");
        ensure!(&mmap[0..4] == MAGIC, "magic inválido (esperado APSH)");
        let version = u32::from_le_bytes(mmap[4..8].try_into()?);
        ensure!(version == VERSION, "versão {version} não suportada");
        let rows = u64::from_le_bytes(mmap[8..16].try_into()?) as usize;
        let cols = u64::from_le_bytes(mmap[16..24].try_into()?) as usize;
        let dtype = u32::from_le_bytes(mmap[24..28].try_into()?);
        if dtype != DTYPE_F64 {
            bail!("dtype {dtype} não suportado nesta fase (apenas f64)");
        }
        ensure!(
            mmap.len() == HEADER_LEN + rows * cols * 8,
            "tamanho do arquivo não bate com rows×cols"
        );
        Ok(Self { mmap, rows, cols })
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Todos os dados, coluna-major. O mmap é alinhado à página e o cabeçalho tem 64 bytes,
    /// então o slice de f64 está sempre alinhado.
    pub fn data(&self) -> &[f64] {
        bytemuck::cast_slice(&self.mmap[HEADER_LEN..])
    }

    pub fn col(&self, j: usize) -> &[f64] {
        &self.data()[j * self.rows..(j + 1) * self.rows]
    }
}

/// Grava `data` (coluna-major, `rows*cols` valores) no formato binário.
pub fn write_bin(path: &Path, rows: usize, cols: usize, data: &[f64]) -> Result<()> {
    ensure!(data.len() == rows * cols, "data.len() != rows*cols");
    let mut w = BufWriter::with_capacity(1 << 20, File::create(path)?);
    let mut header = [0u8; HEADER_LEN];
    header[0..4].copy_from_slice(MAGIC);
    header[4..8].copy_from_slice(&VERSION.to_le_bytes());
    header[8..16].copy_from_slice(&(rows as u64).to_le_bytes());
    header[16..24].copy_from_slice(&(cols as u64).to_le_bytes());
    header[24..28].copy_from_slice(&DTYPE_F64.to_le_bytes());
    w.write_all(&header)?;
    w.write_all(bytemuck::cast_slice(data))?;
    w.flush()?;
    Ok(())
}
