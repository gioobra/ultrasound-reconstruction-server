use serde::{Deserialize, Serialize};

/// Modelos disponíveis. Dimensões conforme `specs.md` §3 (S e N são hipótese H1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelId {
    #[serde(rename = "60x60")]
    M60,
    #[serde(rename = "30x30")]
    M30,
}

impl ModelId {
    pub const ALL: [ModelId; 2] = [ModelId::M60, ModelId::M30];

    pub fn name(self) -> &'static str {
        match self {
            ModelId::M60 => "60x60",
            ModelId::M30 => "30x30",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == s)
    }

    /// Número de linhas de H (= S·N = tamanho do sinal g).
    pub fn rows(self) -> usize {
        self.s() * self.n()
    }

    /// Número de colunas de H (= pixels da imagem).
    pub fn cols(self) -> usize {
        self.width() * self.width()
    }

    pub fn width(self) -> usize {
        match self {
            ModelId::M60 => 60,
            ModelId::M30 => 30,
        }
    }

    /// Amostras por sensor.
    pub fn s(self) -> usize {
        match self {
            ModelId::M60 => 794,
            ModelId::M30 => 436,
        }
    }

    /// Número de sensores.
    pub fn n(self) -> usize {
        64
    }

    /// Nome base dos arquivos de H (`H-1.csv.zip`, `H-1.bin`).
    pub fn h_stem(self) -> &'static str {
        match self {
            ModelId::M60 => "H-1",
            ModelId::M30 => "H-2",
        }
    }
}
