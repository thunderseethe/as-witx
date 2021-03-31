use as_witx_lib::Generator;
use witx::*;
use std::{fs::File, io::Read, path::PathBuf};

use difference::assert_diff;

#[test]
fn test_equivalence() -> Result<(), as_witx_lib::Error> {
    let base: PathBuf = std::env::var("CARGO_MANIFEST_DIR").unwrap().into();
    let gen = Generator::new(None, true)
        .generate(base.join("tests/input/proposal_asymmetric_common.witx"))?;

    let mut f = File::open(base.join("tests/output/proposal_asymmetric_common.ts")).unwrap();
    let mut expected = String::new();
    let _ = f.read_to_string(&mut expected).unwrap();

    //assert_eq!(gen, expected);
    assert_diff(&gen, &expected, "\n", 0);

    Ok(())
}

