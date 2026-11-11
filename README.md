# NUMEROPDF

Aplicação desktop em Rust para organizar PDFs e numerar pranchas. Interface nativa egui; operações PDF e renderização por módulos do [PDFCraft](https://github.com/storytold/pdfcraft), fixados em um submódulo. Sem JavaScript, Electron, Tauri ou serviços remotos de processamento.

**Estado: alpha 0.2.0.** O núcleo foi compilado e testado em Linux; a interface foi compilada, mas ainda requer validação interativa em Windows. Não é uma versão final.

## Implementado

- Abrir vários PDFs por seleção ou arrastar e soltar. Página centralizada e ajustada à tela na abertura.
- Zoom de 10% a 800%, pelo controle ou Ctrl + rolagem, sem bloquear navegação durante processamento.
- Renderizador persistente em segundo plano, cache de imagens de 64 MiB e atraso de 150 ms para agrupar mudanças de zoom. A imagem anterior continua visível durante o refinamento.
- Numerar PDFs em lote, com sequência reiniciada por documento, progresso e relatório de falhas. Arquivos existentes nunca são sobrescritos.
- Mesclar arquivos, extrair intervalos e dividir em um arquivo por página.
- Extrair/reordenar listas de páginas pelo CLI.
- Exportar a página visível em PNG com DPI configurável.
- Reconhecer blocos de texto completos `01/06`, detectar tamanho da fonte e limites da célula fechada (ou os limites do próprio texto), e substituí-los sem cobertura branca. A edição remove o bloco original pelo PDFCraft.
- Numerar um intervalo, escolher número inicial e quantidade de dígitos; incluir/excluir o total, prefixo e separador. Total pode ser quantidade de folhas ou último número.
- Escolher Helvetica, Times ou Courier; tamanho, negrito, itálico e cor. Ajuste de tamanho à área disponível.
- Criar contagem em área manual, desenhada na prévia ou informada em pontos PDF.
- Detectar candidatos a quadrados com X ou +, pelo raster, com cor e tolerância configuráveis. Quadrados preenchidos são rejeitados. O botão **Salvar marcador SVG** cria um símbolo escalável na cor e forma selecionadas.
- Gerar prévia a partir dos mesmos bytes PDF que serão exportados; alternar antes/depois. Alterar configurações invalida a prévia anterior.
- Salvar e carregar padrões JSON; desfazer a última operação de organização.
- Salvar cópias atomicamente sem sobrescrever arquivos existentes.

## Uso

1. Abra um PDF.
2. Selecione o intervalo: por padrão, a primeira folha fica fora quando há mais de uma página.
3. Escolha `Substituir contagem existente`, `Marcador colorido` ou `Área manual`.
4. Configure a sequência e a aparência.
5. Clique **Gerar prévia real**, confira as páginas e use **Salvar cópia**.

A barra de status informa erros. Se houver zero ou vários candidatos numa página, a operação inteira é recusada; não há páginas silenciosamente puladas. Use outro intervalo ou uma área manual para nova contagem. Área manual não apaga contagens existentes.

O modo de marcador requer habilitar explicitamente **cobertura branca**. Nesta versão, o marcador continua no PDF sob a cobertura: não é remoção definitiva nem redaction. Usar apenas em fundo branco. A detecção é raster, experimental, e deve ser revisada antes da exportação. O símbolo editável está em `assets/marcador-roxo.svg` (10 mm, roxo #961EC8); mantenha tamanho suficiente para a contagem.

## Compilar

```sh
git clone --recurse-submodules https://github.com/robsonmoraisdesign-netizen/NUMEROPDF.git
cd NUMEROPDF
rustup update stable
cargo run --release
```

Rust 1.95 ou superior e ferramentas de compilação nativas. No Windows, instalar Rust com toolchain MSVC e Visual Studio Build Tools (C++). O workflow de GitHub Actions gera um pacote portátil Windows x64 após os testes; conferir o resultado em **Actions**, sem presumir aprovação da compilação.

```sh
cargo fmt --check
cargo clippy --all-targets --no-deps -- -D warnings
cargo test --locked
```

## Linha de comando

Sem argumentos, abre a interface. Todos os caminhos de saída precisam ser novos.

```sh
numeropdf inspect entrada.pdf
numeropdf number entrada.pdf numerado.pdf examples/numeracao.json
numeropdf extract entrada.pdf selecionado.pdf 3,1,2
numeropdf merge unido.pdf primeiro.pdf segundo.pdf
numeropdf split entrada.pdf nova-pasta
numeropdf render entrada.pdf pagina.png 2 150
```

No JSON, `first_page` e `last_page` são índices inclusivos começando em **zero**. Na interface e no CLI `render`/`extract`, páginas começam em **um**. `start` é o número impresso, independente da página inicial. Não inserir documentos particulares no repositório público.

## Limitações e próximas etapas

- Sem OCR de contagem em PDFs escaneados; números precisam ser texto editável para substituição automática.
- Seleção entre vários candidatos e múltiplos intervalos com capas intermediárias ainda não estão na interface.
- Marcador detectado por imagem, sem remoção vetorial definitiva. Cores configuráveis e símbolos X/+; símbolos arbitrários ainda não suportados.
- Não há importação de fontes TTF/OTF, miniaturas, exportação de imagens em lote ou histórico completo.
- Zoom renderiza a folha inteira; visualização por blocos para zoom extremo ainda não implementada. Rasters acima dos limites do PDFCraft são recusados.
- PDFs protegidos por senha são recusados. Edições em documentos assinados invalidam assinaturas; usar cópias adequadas.
- Divisão pode deixar páginas já exportadas se ocorrer erro de disco; o erro informa a quantidade concluída.
- Não foi feita validação interativa do executável Windows.

Arquitetura e evidências: [docs/VALIDACAO.md](docs/VALIDACAO.md). Licenças do PDFCraft e dependências são preservadas no submódulo; a aplicação não utiliza marcas visuais ArtCraft.

## Uso das melhorias da versão 0.2

Abra ou solte vários PDFs sobre a janela. No painel **Lote**, selecione o documento para conferir a prévia, ajuste a numeração e use **Numerar lote e salvar…**. A opção de última folha adapta o intervalo ao tamanho de cada documento; o número inicial reinicia em cada arquivo. Um erro é informado e os demais PDFs continuam. O limite da lista é de 1.000 arquivos.

Segure **Ctrl** e gire a roda sobre a página para aproximar ou afastar. A página fica centralizada quando cabe na área disponível; com zoom maior, as barras permitem examinar toda a folha. Ao mudar zoom, a visualização volta ao centro.

Em **Substituir contagem existente**, o tamanho original é detectado por padrão e ajustado para caber. Desmarque **Detectar tamanho da fonte existente** para usar o tamanho manual. A detecção automática desta versão reconhece texto editável no formato `01/06`; não inclui OCR de contagens em imagens. Candidatos ausentes ou ambíguos geram erro para revisão.
