# Arquitetura e validação da alpha

## Base

PDFCraft fixado em `92b965dba324a15c3c3a48e5d7b1e78fb5e4e6bc`, via submódulo.
Usamos `pdfcraft-cos`, `pdfcraft-organize`, `pdfcraft-edit` e `pdfcraft-render` diretamente, sem a interface nem as marcas visuais do upstream. Os patches locais de codecs/renderização apontam para a mesma revisão. O Cargo.lock fixa as dependências.

- `src/lib.rs`: abrir, localizar texto, calcular numeração, remover bloco antigo, inserir novo texto, extrair, mesclar e salvar.
- `src/marker.rs`: componentes conexos por cor, proporção, contorno e duas diagonais; retorna candidatos, não certeza semântica.
- `src/ui.rs`: interface nativa; trabalhos demorados em threads; documento original separado da prévia; invalidação ao mudar parâmetros.
- `src/main.rs`: inicialização desktop e CLI usando o mesmo núcleo.
- `tests/pdf_workflow.rs`: PDFs sintéticos criados em memória, sem arquivos de clientes.

## Evidência local

Ambiente: Linux, Rust 1.99.1.

- `cargo check --all-targets`: passou.
- `cargo fmt --check`: passou.
- `cargo clippy --all-targets --no-deps -- -D warnings`: passou para NUMEROPDF; o upstream tem um aviso de importação não usada em hayro-interpret.
- `cargo test`: 7 testes unitários + 8 de integração passaram.
- Substituição verificou contagem antiga removida, nova contagem extraível, capa intacta e original em memória preservado.
- Extração verificou ordem e mesclagem verificou quantidade.
- Testes incluem página rotacionada, múltiplos candidatos, contagem ausente, tamanho incompatível, overflow e proteção contra sobrescrita.
- Marcador: quadrado sem X e quadrado preenchido são rejeitados; contorno com X é aceito. Padrão azul com + é reconhecido apenas com a configuração correspondente. Teste ponta a ponta exige habilitar a cobertura, insere contagem extraível e verifica que o marcador deixa de aparecer no raster.

Validação privada adicional com cinco PDFs fornecidos pelo usuário: 34 páginas totais, 29 pranchas numeradas e cinco folhas iniciais sem numeração. Renumeração começando em 15 concluída nos cinco documentos. Comparação independente com MuPDF em 72 DPI: zero pixels alterados fora da região reservada ao número em todas as páginas. Recorte ampliado revisado visualmente. Nenhum documento nem dado pessoal foi incluído no repositório.

## Não validado

Interface em uma sessão desktop real; executável no Windows; documentos externos além dos exemplos; OCR; fontes personalizadas; variantes de marcadores além de X/+; remoção definitiva do marcador. A compilação Windows depende do workflow de Actions e deve ser conferida separadamente.

## Critérios para sair de alpha

1. Validar abertura, zoom, seleção de área, prévia e exportação em Windows.
2. Adicionar seleção visual de candidatos, ignorar capas no meio da sequência e grupos por ambiente.
3. Detecção vetorial, remoção de marcador com preservação do fundo.
4. Fontes adicionais, alinhamento configurável, miniaturas e exportação de imagens em lote.
5. Renderização por blocos com cancelamento, cache limitado e comparação em zoom extremo.


## Versão 0.2 — usabilidade e desempenho

- 22 testes automatizados: sequência, segurança de gravação, marcador, organização, remoção do texto anterior, fonte e célula detectadas, lote com falha intermediária, cache do renderizador e entrada da interface.
- Teste de interface egui sem janela verifica página centralizada, Ctrl + roda alterando zoom do PDF sem alterar escala da interface, e arquivos múltiplos enfileirados durante operação.
- Teste do renderizador verifica que redesenhar a interface não provoca nova rasterização, que o parser é mantido entre escalas e que retornar a uma escala em cache não renderiza novamente.
- Lote usa apenas um documento por vez e continua após falhas; todos os arquivos de saída usam gravação sem sobrescrita.
- `cargo test --locked`, `cargo fmt --check` e Clippy do projeto com avisos tratados como erro. Uma advertência de importação não utilizada permanece no fornecedor hayro-interpret, não modificado.
- Ainda é necessária validação interativa no Windows, especialmente arrastar arquivos do Explorer e uso com telas de DPI diferentes. Não há medição de velocidade em máquina do usuário.


## Versão 0.3 — regressão da contagem e múltiplos documentos

28 testes automatizados passaram. Os novos casos cobrem contagem dentro de um parágrafo com outra linha (que deve ser preservada), remoção de dois números sobrepostos, renumeração repetida sem acumular texto, bloqueio de sobreposição manual, seleção de região, duas capas e estado independente das abas ao alternar/fechar.

Validação privada de um documento de 23 páginas: 21 folhas renumeradas com uma única contagem editável em cada folha e duas páginas iniciais intactas. Comparação independente por PyMuPDF não encontrou diferenças de pixels fora da célula da numeração. A página antes não detectada foi renderizada em ampliação para revisão visual. Nenhum PDF de cliente faz parte do repositório.

Testes da interface usam egui sem janela. A interação manual em Windows ainda não foi verificada nesta versão.
