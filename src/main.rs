mod astype;
mod error;
mod generator;
mod pretty_writer;

#[macro_use]
extern crate clap;

use crate::generator::*;
use clap::Arg;
use std::fs::File;
use std::io::Write;

fn main() -> Result<(), std::io::Error> {
    let matches = app_from_crate!()
        .arg(
            Arg::with_name("module_name")
                .short("-m")
                .long("--module-name")
                .value_name("module_name")
                .help("Set the module name to use instead of reading it from the witx file"),
        )
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
                .help("wITX file"),
        )
        .arg(
            Arg::with_name("no_embedded_header")
                .multiple(false)
                .required(false)
                .long("--no-embed-header")
                .help("Disable embedded common set of types needed by generated code within output (as opposed to importing form a common file)")
        )
        .get_matches();

    let witx_file = matches.value_of("witx_file").unwrap();
    let module_name = matches.value_of("module_name").map(|x| x.to_string());
    let embed_header = matches.occurrences_of("no_embedded_header") == 0;
    let output = 
        Generator::new(module_name, embed_header)
            .generate(witx_file).expect("generate() failed");

    let mut writer: Box<dyn Write> = match matches.value_of("output_file") {
        None | Some("-") => Box::new(std::io::stdout()),
        Some(file) => Box::new(File::create(file).unwrap()),
    };
    writer.write_all(output.as_ref())
}
