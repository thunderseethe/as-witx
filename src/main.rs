
#[macro_use]
extern crate clap;

use as_witx_lib::generate;
use clap::Arg;
use std::fs::File;
use std::io::Write;

fn main() -> Result<(), std::io::Error> {
    let matches = app_from_crate!()
        .arg(
            Arg::with_name("output_file")
                .short("-o")
                .long("--output")
                .value_name("output_file")
                .multiple(false)
                .help("Output file, or - for the standard output"),
        )
        .arg(
            Arg::with_name("witx_file")
                .multiple(false)
                .required(true)
                .help("WITX file"),
        )
        .get_matches();

    let witx_file = matches.value_of("witx_file").unwrap();
    let output = generate(witx_file).expect("generate() failed");

    let mut writer: Box<dyn Write> = match matches.value_of("output_file") {
        None | Some("-") => Box::new(std::io::stdout()),
        Some(file) => Box::new(File::create(file).unwrap()),
    };
    writer.write_all(output.as_ref())
}
