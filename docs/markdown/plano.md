# Plano de Implementação — Reconstrução de Imagens (APS semestral)

Escopo: **servidor, cliente e relatórios em Rust** (versão compilada). Detalhes técnicos em [`arquitetura.md`](arquitetura.md) e [`specs.md`](specs.md).

A ordem das fases prioriza ter uma reconstrução **correta** de ponta a ponta antes de otimizar. Cada fase termina com algo executável.

---

## Fase 0 — Preparação e validação dos dados

Objetivo: confirmar as hipóteses sobre os dados antes de escrever o sistema.

- [x] Criar o workspace Cargo (por ora só `aps-core` e `aps-tools`; os demais crates entram nas fases que os usam)
- [x] `aps-tools prepare`: ler `H-*.csv` direto do `.zip` (streaming), validar as dimensões, gravar `cache/H-*.bin` (coluna-major, cabeçalho `APSH`)
- [x] Medir a densidade de `H-1` (a de `H-2` é ~25%) para confirmar a escolha de matriz densa
- [x] Calcular `c = ‖HᵀH‖₂` por iteração de potência e salvar `cache/c-*.json`
- [x] `aps-tools reconstruct`: CGNR simples (sem servidor) sobre `G-1.csv` → PNG
- [x] Validar visualmente **H3** (orientação da imagem: confirmada com as imagens de `docs/`; ver `specs.md` §3)
- [x] Validar **H1/H2** (S, N e layout de `g`): aplicar o ganho em `G-1` e comparar com `A-60x60-1.csv` (**H4** se mostrou falsa)
- [x] Verificar a ordem de grandeza de ε nas primeiras iterações, com e sem ganho (ver risco R1)

**Entrega:** ✅ `cache/` gerado (1,6 GB), PNGs de `G-1`, `G-2`, `g-30x30-1` reconstruídos, hipóteses H1–H4 resolvidas no `specs.md` §3. H3 confirmada com as imagens de `docs/`.

---

## Fase 1 — Núcleo (`aps-core`)

- [ ] `Model`: carrega `H-*.bin` via `memmap2` e expõe `MatRef<f64>` do `faer` sem cópia; guarda `rows`, `cols`, `width`, `S`, `N`, `c`
- [ ] `cgne(model, g, cfg) -> Reconstruction` e `cgnr(...)`, conforme specs §4
- [ ] Critério de parada (ε, máximo de iterações, breakdown) e `stop_reason`
- [ ] Cálculo de `λ` aproveitando `Hᵀg`
- [ ] `gain::apply(g, S, N)`
- [ ] `image::to_png(f, width, scale)`
- [ ] Testes unitários (specs §9.1, exceto o de lote)

**Entrega:** `cargo test -p aps-core` verde; `aps-tools reconstruct` usando o núcleo.

---

## Fase 2 — Servidor básico (`aps-protocol` + `aps-server`)

- [ ] Tipos do contrato (`ReconstructQuery`, `ReconstructResponse`, `ErrorBody`, `Health`, `Stats`) em `aps-protocol`
- [ ] axum: `POST /v1/reconstruct`, `GET /v1/health`, `GET /v1/stats`
- [ ] Validação do tamanho do corpo e conversão `bytes → Vec<f64>` (`bytemuck` / `f64::from_le_bytes`)
- [ ] Fila FIFO com capacidade, `oneshot` de resposta, `queue_len` atômico
- [ ] Pool de `K` workers em threads de SO (ainda sem batching, `B = 1`)
- [ ] Timestamps `received_at`, `started_at`, `finished_at` (RFC 3339 com ms)
- [ ] Teste de integração: sobe o servidor numa porta efêmera e faz um POST com `g-30x30-1` → 200 + 900 pixels

**Entrega:** `curl --data-binary @g.bin "…/v1/reconstruct?…"` devolve a imagem.

---

## Fase 3 — Cliente (`aps-client`)

- [ ] Geração do plano determinístico (`ChaCha8Rng`) + `plan.json`, com opção `--plan` para reutilizar
- [ ] Cache dos sinais-base em memória (lidos uma vez por arquivo)
- [ ] N tasks de cliente; envios disparados em tasks separadas (sem esperar a resposta anterior)
- [ ] Modo `--algorithm cgne|cgnr|both` (no `both`, as fases são sequenciais com o mesmo plano)
- [ ] Gravação de `results.jsonl`, `images/*.png`, `window.json`
- [ ] `relatorio-imagens.html` (specs §7.1)
- [ ] Tratamento de `503`/timeout: registra a falha e segue

**Entrega:** execução completa com 3 clientes × 15 sinais × 2 algoritmos gerando os dois relatórios de imagens.

---

## Fase 4 — Métricas e relatório comparativo

- [ ] Amostrador no servidor (`sysinfo`): CPU%, RSS, `queue_len`, `in_progress`, `completed_total` → `metrics.csv`
- [ ] `aps-report`: recorte das métricas pela janela de cada fase
- [ ] Cálculos: total de imagens, tempo total, média e moda da fila, tempo médio de reconstrução, extras
- [ ] Gráficos SVG de CPU e memória (`plotters`), com uma série por fase
- [ ] `relatorio-comparativo.html`
- [ ] Teste: métricas e resultados sintéticos → valores de média e moda conferidos à mão

**Entrega:** relatório comparativo CGNE × CGNR completo (critérios de aceite §9.3 do `specs.md`).

---

## Fase 5 — Otimização ("maior número de imagens no menor tempo")

Medir sempre com `aps-tools bench` e com uma execução completa do cliente, usando a mesma semente. Guardar os números em uma tabela no README.

- [ ] **Baseline:** `K = 1`, `T = 16`, `B = 1`, f64
- [ ] **Batching:** implementar o CG em lote com compactação de colunas convergidas; teste de equivalência lote × individual (specs §9.1)
- [ ] Varredura de `B ∈ {1, 2, 4, 8}`, `K ∈ {1, 2, 4}`, `T = 16 / K`, `batch_window_ms ∈ {0, 5, 20}`
- [ ] Avaliar f32: tempo, diferença máxima na imagem e iterações vs f64; adotar só se for equivalente
- [ ] Build `--release` com `lto = "fat"`, `codegen-units = 1`, `target-cpu=native` (documentar que o binário fica específico da máquina)
- [ ] Perfil (`perf` / `cargo flamegraph`) para confirmar que o tempo está nas matvecs
- [ ] Fixar os valores padrão finais de `K`, `T` e `B` no servidor

**Entrega:** tabela de desempenho antes/depois e configuração padrão justificada.

---

## Fase 6 — Fechamento

- [ ] README: como preparar os dados, subir o servidor, rodar o cliente e gerar os relatórios
- [ ] Execução final oficial (semente registrada) e arquivamento de `runs/<run_id>/`
- [ ] Atualizar `arquitetura.md` e `specs.md` com o que mudou durante a implementação
- [ ] Revisão do código (`cargo clippy -- -D warnings`, `cargo fmt --check`)

---

## Pontos em aberto

| ID | Questão | Padrão adotado até decidir |
|---|---|---|
| P1 | ~~O que é aleatório no ganho?~~ **Decidido (usuário):** sortear **se** o ganho é aplicado. | Bernoulli(0,5) por sinal, registrado em `gain_applied`. Observação: as imagens de referência parecem usar ganho, e sem ganho a imagem sai pior (para na iteração 2–3). Trocar por "sempre aplica" é uma linha no sorteio. |
| P2 | ~~Orientação da imagem (H3)~~ **Resolvido:** confirmada com as imagens de referência (`docs/`). | `pixel(lin, col) = f[col·W + lin]`. |
| P3 | `c` e `λ` aparecem nas definições, mas não nos algoritmos | Calcular e registrar nos metadados, sem alterar as iterações. Confirmar se o professor espera uma variante regularizada. |
| P4 | ~~O que são os `A-*.csv`?~~ **Resolvido:** são arquivos genéricos, sem imagem de referência. | Ignorados no sistema. |

## Riscos

| ID | Risco | Mitigação |
|---|---|---|
| R1 | **Confirmado:** sem ganho, ε < 1e-4 já na iteração 2–3 (imagem fraca); com ganho, nunca converge e sempre roda 10 iterações | **Decidido:** critério literal (ε < 1e-4 absoluto), por ser o que reconstrói mais rápido. Documentar no relatório. |
| R2 | `H-1` ocupa 1,46 GB; com f64 + mmap cabe com folga em 15 GB, mas cópias acidentais por requisição estouram a memória | `Arc<Model>` com mmap; nenhum worker copia `H`; teste de RSS na fase 4. |
| R3 | Benchmarks variam por turbo, temperatura e outros processos | Repetir cada configuração 3× e reportar mediana; fechar outros programas durante a execução oficial. |
| R4 | Muitas requisições abertas ao mesmo tempo esgotam o timeout | Timeout de 10 min no cliente; `queue-capacity` alto; `503` explícito. |
