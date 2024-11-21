extern crate clap;
extern crate image;

mod export;
mod import;
mod puzzle;
mod solve;

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
        .get_matches();
    let img = image::open(matches.value_of("INPUT").unwrap()).unwrap();

    let puzzle = import::image_to_puzzle(&img);

    let output = if matches.is_present("olsak") {
        export::emit_olsak(&puzzle)
    } else {
        export::emit_webpbn(&puzzle)
    };

    match matches.value_of("OUTPUT") {
        None => {
            print!("{}", output);
        }
        Some(filename) => {
            std::fs::write(filename, output)?;
        }
    }

    Ok(())
}
