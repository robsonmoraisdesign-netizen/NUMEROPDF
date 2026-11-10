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
- `cargo test`: 6 testes unitários + 7 de integração passaram.
- Substituição verificou contagem antiga removida, nova contagem extraível, capa intacta e original em memória preservado.
- Extração verificou ordem e mesclagem verificou quantidade.
- Testes incluem página rotacionada, múltiplos candidatos, contagem ausente, tamanho incompatível, overflow e proteção contra sobrescrita.
- Marcador: quadrado sem X e quadrado preenchido são rejeitados; contorno com X é aceito.

Validação privada adicional com cinco PDFs fornecidos pelo usuário: 34 páginas totais, 29 pranchas numeradas e cinco folhas iniciais sem numeração. Renumeração começando em 15 concluída nos cinco documentos. Comparação independente com MuPDF em 72 DPI: zero pixels alterados fora da região reservada ao número em todas as páginas. Recorte ampliado revisado visualmente. Nenhum documento nem dado pessoal foi incluído no repositório.

## Não validado

Interface em uma sessão desktop real; executável no Windows; documentos externos além dos exemplos; OCR; fontes personalizadas; variantes de marcadores; remoção definitiva do marcador. A compilação Windows depende do workflow de Actions e deve ser conferida separadamente.

## Critérios para sair de alpha

1. Validar abertura, zoom, seleção de área, prévia e exportação em Windows.
2. Adicionar seleção visual de candidatos, ignorar capas no meio da sequência e grupos por ambiente.
3. Detecção vetorial, remoção de marcador com preservação do fundo e cores configuráveis.
4. Fontes adicionais, alinhamento configurável, miniaturas e exportação de imagens em lote.
5. Renderização por blocos com cancelamento, cache limitado e comparação em zoom extremo.
