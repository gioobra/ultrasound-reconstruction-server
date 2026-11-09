# Especificação — Reconstrução de Imagens (APS semestral)

Especificação funcional e técnica da implementação em **Rust**. A arquitetura está em [`arquitetura.md`](arquitetura.md) e o plano de execução em [`plano.md`](plano.md).

---

## 1. Glossário

| Símbolo | Significado |
|---|---|
| **g** | vetor de sinal (tamanho S·N) |
| **H** | matriz de modelo (S·N × pixels) |
| **f** | imagem reconstruída (vetor de tamanho = pixels) |
| **S** | número de amostras do sinal |
| **N** | número de elementos sensores |
| **c** | fator de redução |
| **λ** | coeficiente de regularização |
| **ε** | erro (critério de parada) |
| **γ** | ganho de sinal |

---

## 2. Requisitos (do enunciado)

### 2.1 Cliente

| ID | Requisito | Como será atendido |
|---|---|---|
| RC1 | Iniciar simultaneamente no mínimo 3 clientes | `--clients N` (padrão 3), uma task tokio por cliente |
| RC2 | Enviar no mínimo 15 sinais por cliente em intervalos aleatórios | `--signals M` (padrão 15); intervalo uniforme em `[--min-interval-ms, --max-interval-ms]` |
| RC3 | Ganho de sinal e modelo da imagem definidos aleatoriamente | sorteados no plano a partir da semente (ver §5) |
| RC4 | Ganho implementado no cliente | `aps-core::gain` chamado pelo cliente antes do envio |
| RC5 | Relatório com todas as imagens e as informações de cada uma | `relatorio-imagens.html` por fase |
| RC6 | Mesma sequência de sinais para CGNE e CGNR | plano determinístico por `--seed`; modo `--algorithm both` reutiliza o mesmo plano |

### 2.2 Servidor

| ID | Requisito | Como será atendido |
|---|---|---|
| RS2 | Versão compilada e fortemente tipada | Rust |
| RS3 | Executar o algoritmo de reconstrução | CGNE e CGNR em `aps-core` |
| RS4 | Parar quando ε < 1e-4 ou ao chegar a 10 iterações | §4.3 |
| RS5 | Relatório comparativo | `aps-report` (§7) |
| RS6 | Reconstruir o maior número de imagens no menor tempo | `H` em mmap, paralelismo, batching (ver arquitetura §5.3) |

> RS1 (versão interpretada) não faz parte deste escopo.

### 2.3 Metadados obrigatórios de cada imagem

- Identificação do cliente que enviou → `client_id`
- Identificação do sinal enviado → `signal_id`
- Identificação do algoritmo utilizado → `algorithm`
- Data e hora do início da reconstrução → `started_at`
- Data e hora do término da reconstrução → `finished_at`
- Número de iterações executadas → `iterations`

---

## 3. Modelos e dados

| `model` | H | linhas (S·N) | colunas (pixels) | imagem | S | N |
|---|---|---|---|---|---|---|
| `60x60` | `H-1` | 50816 | 3600 | 60 × 60 | 794 | 64 |
| `30x30` | `H-2` | 27904 | 900 | 30 × 30 | 436 | 64 |

Sinais-base disponíveis em `data/`:

| `model` | arquivos |
|---|---|
| `60x60` | `G-1.csv`, `G-2.csv` |
| `30x30` | `g-30x30-1.csv`, `g-30x30-2.csv` |

Os CSVs de sinal têm um valor por linha. Os CSVs de H têm uma linha da matriz por linha do arquivo, com valores separados por vírgula.

**Hipóteses validadas na fase 0** (medidas com `aps-tools`):
- **H1 ✅ confirmada:** S e N são os da tabela. As dimensões de H batem exatamente (50816 × 3600 e 27904 × 900) e as reconstruções ficam coerentes.
- **H2 ✅ confirmada (por coerência visual):** `g` está organizado por sensor, `idx = c·S + (l−1)` (as S amostras do sensor 0, depois as do sensor 1, …). Com o ganho aplicado nesse layout, `G-2` (60×60) reconstrói uma grade regular de refletores bem definida.
- **H3 ✅ confirmada com as imagens de referência** (`docs/G1.png`, `G2.png`, `g1.png`, `g2.png`): `pixel(lin, col) = f[col·W + lin]`, sem transposição, com a coluna no eixo x e a linha no eixo y. Os pontos de `g-30x30-2` caem nas mesmas posições da referência (±1 pixel, que vem do eixo 1-based do MATLAB), e a grade de `G-2` tem a mesma forma, com a largura aumentando para baixo. As referências usam escala log (`G`) e ABS (`g`); o nosso PNG é linear normalizado, então só a posição é comparável.
- **H4 ❌ falsa:** os `A-*.csv` **não** são `g` com ganho aplicado. `check-gain` dá erro relativo 1,0 nos dois layouts, e reconstruir `A-30x30-1` quase não reduz o resíduo (‖r‖ cai só ~10% em 10 iterações), ou seja, o sinal não é consistente com `H-2`. Confirmado pelo usuário: são arquivos genéricos, sem imagem de referência. Não são usados no sistema.

**Medições da fase 0**

| | `60x60` | `30x30` |
|---|---|---|
| Densidade de H | 15,11% não-zeros | 25,37% não-zeros |
| `c = ‖HᵀH‖₂` | 8,416·10⁻⁸ | 8,301·10⁻⁸ |
| λ típico (com ganho) | ~5,4–5,9·10⁻⁷ | ~5,5·10⁻⁷ |
| Reconstrução, 10 iterações (versão simples) | ~1,2 s | ~0,17 s |
| Tamanho de `cache/H-*.bin` | 1,46 GB | 0,20 GB |

**Comportamento do critério de parada (ε < 1e-4), CGNR:**
- **Sem ganho** (`G-*`, `g-*`, valores ~1e-5): ε já nasce na casa de 1e-4, então para na iteração **2–3** (`converged`) e a imagem fica fraca.
- **Com ganho:** ε começa em ~3·10⁻² e em 10 iterações ainda está em ~2–3·10⁻³, então **sempre** roda as 10 iterações (`max_iterations`).
- **Decisão (usuário):** manter o critério literal (absoluto), por ser o que reconstrói mais rápido.
- As imagens de referência parecem ter sido geradas **com** ganho: na referência de `G-2` o brilho cresce para baixo, como na nossa reconstrução com ganho. Sem ganho, todos os pontos saem com o mesmo brilho.
- Consequência: o sorteio de `apply_gain` define praticamente a carga de cada requisição (2–3 iterações contra 10). Isso explica boa parte da variância de tempo no relatório e deve ser mencionado nele.

---

## 4. Algoritmos

Notação: `i` é o índice da iteração; ‖·‖₂ é a norma euclidiana; todas as operações são em f64.

### 4.1 CGNE (Conjugate Gradient Method Normal Error)

```
f₀ = 0
r₀ = g − H·f₀            (= g)
p₀ = Hᵀ·r₀
para i = 0, 1, … até convergir:
    αᵢ   = (rᵢᵀ rᵢ) / (pᵢᵀ pᵢ)
    fᵢ₊₁ = fᵢ + αᵢ pᵢ
    rᵢ₊₁ = rᵢ − αᵢ H pᵢ
    βᵢ   = (rᵢ₊₁ᵀ rᵢ₊₁) / (rᵢᵀ rᵢ)
    pᵢ₊₁ = Hᵀ rᵢ₊₁ + βᵢ pᵢ
```

Custo por iteração: 1 × `H·v` + 1 × `Hᵀ·v` + operações O(linhas + colunas).

### 4.2 CGNR (Conjugate Gradient Normal Residual), Saad 2003, p. 266

```
f₀ = 0
r₀ = g − H·f₀            (= g)
z₀ = Hᵀ·r₀
p₀ = z₀
para i = 0, 1, … até convergir:
    wᵢ   = H pᵢ
    αᵢ   = ‖zᵢ‖₂² / ‖wᵢ‖₂²
    fᵢ₊₁ = fᵢ + αᵢ pᵢ
    rᵢ₊₁ = rᵢ − αᵢ wᵢ
    zᵢ₊₁ = Hᵀ rᵢ₊₁
    βᵢ   = ‖zᵢ₊₁‖₂² / ‖zᵢ‖₂²
    pᵢ₊₁ = zᵢ₊₁ + βᵢ pᵢ
```

Custo por iteração: igual ao do CGNE.

### 4.3 Critério de parada

```
εᵢ = | ‖rᵢ₊₁‖₂ − ‖rᵢ‖₂ |
para se εᵢ < 1e-4                           → stop_reason = "converged"
para se iterações executadas = 10           → stop_reason = "max_iterations"
para se aparecer divisão por zero ou NaN/Inf → stop_reason = "breakdown"
```

- O enunciado escreve "1e10⁻⁴"; a leitura adotada é **10⁻⁴**.
- Usa-se o valor absoluto da diferença, porque o resíduo pode subir ou descer entre iterações.
- `iterations` = número de vezes que o corpo do laço executou (1 a 10).
- Limiar e máximo são configuráveis no servidor (`--epsilon`, `--max-iterations`), com padrão `1e-4` e `10`.

### 4.4 Fator de redução e coeficiente de regularização

```
c = ‖Hᵀ·H‖₂                      (norma espectral = σ_max(H)²)
λ = max(|Hᵀ·g|) · 0.10
```

- `c` é calculado uma vez por modelo no `prepare`, por iteração de potência sobre `HᵀH` (tolerância relativa 1e-6, máximo de 100 passos), e salvo em `cache/c-<modelo>.json`.
- `λ` é calculado por sinal e sai de graça: `Hᵀ·g` é o próprio `p₀` (CGNE) ou `z₀` (CGNR).
- Os dois valores são **registrados nos metadados**. Os algoritmos do enunciado não os usam, então eles não alteram as iterações (ponto em aberto P3 no `plano.md`).

### 4.5 Ganho de sinal (no cliente)

```
para c = 1..N:
    para l = 1..S:
        γₗ = 100 + (1/20) · l · √l
        g[l, c] = g[l, c] · γₗ
```

Em Rust, com índices 0-based e o layout H2: `g[c·S + (l−1)] *= 100.0 + (l as f64) * (l as f64).sqrt() / 20.0` para `l` em `1..=S`.

### 4.6 Conversão de `f` para PNG

1. Reorganizar `f` em W × W conforme H3.
2. Normalizar para `[0, 255]`: `px = 255 · (v − min) / (max − min)`. Se `max = min`, a imagem fica toda preta.
3. Salvar em escala de cinza, ampliada 4× (vizinho mais próximo) para ficar legível no relatório.

---

## 5. Plano de envio (cliente)

O plano é gerado de forma determinística com `ChaCha8Rng::seed_from_u64(seed)`. Para cada cliente `k ∈ [1, N]` e sinal `j ∈ [1, M]`:

| Campo | Sorteio |
|---|---|
| `model` | uniforme entre `60x60` e `30x30` |
| `base_file` | uniforme entre os arquivos sem ganho do modelo (`G-1`/`G-2` ou `g-30x30-1`/`g-30x30-2`) |
| `apply_gain` | Bernoulli(0,5) (decisão do usuário, ver `plano.md` P1) |
| `delay_ms` | uniforme em `[min_interval_ms, max_interval_ms]` (padrão 100–2000) |
| `signal_id` | `"c{k:02}-s{j:02}"` |

`plan.json`:

```json
{
  "seed": 42,
  "clients": 3,
  "signals_per_client": 15,
  "entries": [
    { "client_id": "c01", "signal_id": "c01-s01", "model": "60x60",
      "base_file": "G-2.csv", "apply_gain": true, "delay_ms": 734 }
  ]
}
```

---

## 6. Contrato HTTP

Base: `http://<host>:<porta>` (padrão `127.0.0.1:8080`). Os tipos ficam no crate `aps-protocol`.

### 6.1 `POST /v1/reconstruct`

**Query string**

| Parâmetro | Tipo | Obrigatório | Exemplo |
|---|---|---|---|
| `run_id` | string | sim | `2026-11-09T17-02-11_seed42` |
| `client_id` | string | sim | `c01` |
| `signal_id` | string | sim | `c01-s07` |
| `model` | `60x60` \| `30x30` | sim | `60x60` |
| `algorithm` | `cgne` \| `cgnr` | sim | `cgnr` |
| `gain_applied` | bool | sim | `true` |

**Headers:** `Content-Type: application/octet-stream`

**Corpo:** `rows(model)` valores **f64 little-endian**, sem cabeçalho (50816 × 8 = 406 528 bytes para `60x60`; 27904 × 8 = 223 232 bytes para `30x30`).

**Resposta `200 OK`** (`application/json`):

```json
{
  "run_id": "2026-11-09T17-02-11_seed42",
  "client_id": "c01",
  "signal_id": "c01-s07",
  "model": "60x60",
  "algorithm": "cgnr",
  "gain_applied": true,
  "received_at": "2026-11-09T17:02:14.120Z",
  "started_at":  "2026-11-09T17:02:14.388Z",
  "finished_at": "2026-11-09T17:02:15.402Z",
  "queue_wait_ms": 268,
  "reconstruction_ms": 1014,
  "iterations": 7,
  "stop_reason": "converged",
  "final_epsilon": 8.3e-5,
  "final_residual_norm": 0.0123,
  "lambda": 0.0457,
  "c": 2.31e4,
  "batch_size": 3,
  "width": 60,
  "height": 60,
  "image": [0.0, 0.0012, "… 3600 valores f64, coluna-major …"]
}
```

**Erros** (`application/json`, `{ "error": "<mensagem>" }`):

| Status | Quando |
|---|---|
| `400` | parâmetro ausente ou inválido; tamanho do corpo ≠ `rows × 8` |
| `503` | fila cheia (`--queue-capacity`) |
| `500` | erro interno inesperado |

O cliente usa timeout de 10 min por requisição, porque a resposta só chega após a reconstrução.

### 6.2 `GET /v1/health`

```json
{ "status": "ok", "models": ["60x60", "30x30"], "precision": "f64", "workers": 2, "max_batch": 4 }
```

### 6.3 `GET /v1/stats`

```json
{ "queue_len": 5, "in_progress": 4, "completed_total": 87, "uptime_ms": 123456 }
```

---

## 7. Relatórios

### 7.1 Relatório de imagens (cliente, por fase)

`runs/<run_id>/<algoritmo>/relatorio-imagens.html`: grade de cartões, um por imagem, ordenados por `client_id` e `signal_id`. Cada cartão traz:
- a imagem PNG embutida (base64);
- cliente, sinal, algoritmo, modelo, com ou sem ganho;
- início, término, duração e tempo de fila;
- iterações, `stop_reason`, ε final.

### 7.2 Relatório comparativo (`aps-report`)

`runs/<run_id>/relatorio-comparativo.html`, com uma coluna por fase (CGNE, CGNR):

| Métrica | Definição |
|---|---|
| Nº total de imagens reconstruídas | linhas de `results.jsonl` com `200` |
| Tempo total de execução | `max(finished_at) − min(envio)` da fase |
| Gráfico de CPU | `cpu_percent` × tempo (bruto e ÷ núcleos), dentro da janela da fase |
| Gráfico de memória | `rss_bytes` (MB) × tempo, dentro da janela da fase |
| Nº médio de imagens na fila | média de `queue_len` nas amostras da janela |
| Moda de imagens na fila | valor de `queue_len` mais frequente nas amostras da janela (empate → o menor) |
| Tempo médio de reconstrução | média de `reconstruction_ms` |
| Extras | tempo médio de fila, média de iterações, % `converged`, imagens/s, tabela por modelo |

Os gráficos são SVG (`plotters`) embutidos inline. O HTML não depende de nenhum recurso externo.

---

## 8. Interfaces de linha de comando

```bash
# Converte os CSVs de H para binário e calcula c (executar uma vez)
aps-tools prepare --data data/ --out cache/ [--precision f64|f32]

# Servidor
aps-server --cache cache/ --bind 127.0.0.1:8080 \
           --workers 2 --threads-per-worker 8 --max-batch 4 --batch-window-ms 5 \
           --queue-capacity 1000 --epsilon 1e-4 --max-iterations 10 \
           --metrics-interval-ms 100 --out runs/

# Cliente
aps-client --server http://127.0.0.1:8080 --data data/ --out runs/ \
           --clients 3 --signals 15 --seed 42 --algorithm both \
           --min-interval-ms 100 --max-interval-ms 2000 [--plan plan.json]

# Relatório comparativo
aps-report --run runs/<run_id> --metrics runs/server-<ts>/metrics.csv

# Validação e benchmark
aps-tools reconstruct --cache cache/ --model 60x60 --signal data/G-1.csv --algorithm cgnr --png out.png
aps-tools bench --cache cache/ --model 60x60 --batch 1,2,4,8 --threads 1,4,8,16
```

---

## 9. Critérios de aceite

1. `cargo test --workspace` passa, incluindo:
   - CGNE e CGNR resolvem um sistema pequeno conhecido (ex.: H 6×4 de posto completo) com erro < 1e-8 sem limite de iterações;
   - com `max_iterations = 10`, o laço nunca passa de 10;
   - o ganho aplicado em um vetor de uns devolve exatamente γₗ em cada posição;
   - o plano gerado com a mesma semente é idêntico byte a byte;
   - a reconstrução em lote (B = 4) dá o mesmo resultado que 4 reconstruções individuais (tolerância 1e-10).
2. `aps-tools reconstruct` com `G-2.csv` e ganho gera uma imagem 60×60 reconhecível (validação visual de H2 e H3, comparando com `docs/G*.png`).
3. Uma execução completa com `--clients 3 --signals 15 --algorithm both` produz 90 imagens (45 por algoritmo), os dois relatórios de imagens e o relatório comparativo, sem erros.
4. Cada imagem do relatório tem os 6 metadados obrigatórios (§2.3).
5. As duas fases usam a mesma sequência: os `plan.json` são idênticos e os pares `(signal_id, model, base_file, apply_gain)` são iguais.
