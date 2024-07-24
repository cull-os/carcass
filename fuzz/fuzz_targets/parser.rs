#![no_main]

use std::{
    env,
    fs,
    hash::{
        self,
        Hash,
        Hasher,
    },
    hint,
    path,
};

use cab::syntax::parse;
use libfuzzer_sys::fuzz_target;
use yansi::{
    Condition,
    Paint as _,
};

fuzz_target!(|data: &str| {
    let parse = hint::black_box(parse(data));

    if env::var("FUZZ_PARSER_SAVE_VALID")
        .is_ok_and(|value| !matches!(value.as_ref(), "" | "0" | "false"))
    {
        yansi::whenever(Condition::TTY_AND_COLOR);

        if let Ok(root) = parse.result() {
            let syntax = format!("{syntax:#?}", syntax = *root);

            let hash = {
                let mut hasher = hash::DefaultHasher::new();
                syntax.hash(&mut hasher);
                hasher.finish()
            };

            let (data_file, syntax_file) = {
                let base = format!("{hash:x}");

                println!(
                    "Found a valid parse! Wrote it to {base}.",
                    base = base.green().bold()
                );

                (base.clone() + ".cab", base + ".expect")
            };

            let root = path::Path::new("cab-syntax/test/data");

            fs::create_dir_all(root).unwrap();

            fs::write(root.join(data_file), data).unwrap();
            fs::write(root.join(syntax_file), syntax).unwrap();
        }
    }
});
