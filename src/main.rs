extern crate clap;
extern crate image;

mod export;
mod grid_solve;
mod import;
mod line_solve;
mod puzzle;

fn main() -> std::io::Result<()> {
    let matches = clap::App::new("convert-nonogram")
        .version("0.1.2")
        .author("Paul Stansifer")
        .about("Converts images of nonogram solutions to puzzles")
        .arg(
            clap::Arg::with_name("INPUT")
                .help("image input file")
                .required(true),
        )
        .arg(
            clap::Arg::with_name("OUTPUT")
                .long("output")
                .short("o")
                .takes_value(true)
                .help("output file (outputs to stdout if omitted)"),
        )
        .arg(
            clap::Arg::with_name("olsak")
                .long("olsak")
                .help("emit nonogram in the 'olsak' format"),
        )
        .arg(
            clap::Arg::with_name("solve")
                .long("solve")
                .short("s")
                .help("solve the nonogram"),
        )
        .get_matches();
    let img = image::open(matches.value_of("INPUT").unwrap()).unwrap();

    let puzzle = import::image_to_puzzle(&img);

    let output = if matches.is_present("olsak") {
        export::emit_olsak(&puzzle)
    } else {
        export::emit_webpbn(&puzzle)
    };

    if let Some(filename) = matches.value_of("OUTPUT") {
        if filename == "-" {
            print!("{}", output);
        } else {
            std::fs::write(filename, output)?;
        }
    }

    if matches.is_present("solve") {
        grid_solve::solve(&puzzle).unwrap();
    }

    Ok(())
}
