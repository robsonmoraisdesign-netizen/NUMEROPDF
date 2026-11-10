#![forbid(unsafe_code)]
mod ui;
use numeropdf::{Numbering, Pdf};
use std::path::Path;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        if let Err(e) = cli(&args) {
            eprintln!("NUMEROPDF: {e}");
            std::process::exit(1);
        }
        return;
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 850.0])
            .with_min_inner_size([960.0, 650.0]),
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        "NUMEROPDF",
        options,
        Box::new(|cc| Ok(Box::new(ui::App::new(cc)))),
    ) {
        eprintln!("{e}");
    }
}
fn cli(a: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match a.first().map(String::as_str){
        Some("inspect") if a.len()==2=>{let p=Pdf::open(Path::new(&a[1]))?;println!("{} páginas",p.pages.len());for i in 0..p.pages.len(){for c in p.candidates(i)?{println!("Página {}: {} {:?}",i+1,c.text,c.area);}}}
        Some("number") if a.len()==4=>{let p=Pdf::open(Path::new(&a[1]))?;let c:Numbering=serde_json::from_slice(&std::fs::read(&a[3])?)?;p.numbered(&c)?.save(Path::new(&a[2]))?;}
        Some("extract") if a.len()==4=>{let p=Pdf::open(Path::new(&a[1]))?;let indices=a[3].split(',').map(|v|v.parse::<usize>().ok().and_then(|n|n.checked_sub(1)).ok_or("Páginas começam em 1")).collect::<Result<Vec<_>,_>>()?;p.extract(&indices)?.save(Path::new(&a[2]))?;}
        Some("merge") if a.len()>=4=>{let mut p=Pdf::open(Path::new(&a[2]))?;for input in &a[3..]{p=p.merge(&Pdf::open(Path::new(input))?)?;}p.save(Path::new(&a[1]))?;}
        Some("render") if a.len()==5=>{let p=Pdf::open(Path::new(&a[1]))?;let page=a[3].parse::<usize>()?.checked_sub(1).ok_or("Página inválida")?;let dpi=a[4].parse::<f32>()?;let image=p.render(page,dpi/72.0)?;let mut buf=std::io::Cursor::new(Vec::new());image.write_to(&mut buf,image::ImageFormat::Png)?;numeropdf::save_new(Path::new(&a[2]),buf.get_ref())?;}
        Some("split") if a.len()==3=>{let p=Pdf::open(Path::new(&a[1]))?;let dir=Path::new(&a[2]);std::fs::create_dir(dir)?;for i in 0..p.pages.len(){p.extract(&[i])?.save(&dir.join(format!("pagina-{:04}.pdf",i+1)))?;}}
        _=>return Err("Uso: numeropdf [inspect entrada.pdf | number entrada.pdf saida.pdf config.json | extract entrada.pdf saida.pdf 1,3,2 | merge saida.pdf a.pdf b.pdf | render entrada.pdf pagina.png 1 150 | split entrada.pdf nova-pasta]. Sem argumentos: interface gráfica.".into())
    }
    Ok(())
}
