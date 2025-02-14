#![no_main]

use std::{
    env,
    fs,
    hash::{
        self,
        Hash as _,
        Hasher as _,
    },
    path,
};

use cab::{
    report,
    syntax,
};
use libfuzzer_sys::{
    Corpus,
    fuzz_target,
};
use yansi::Paint as _;

fuzz_target!(|data: &str| -> Corpus {
    let mut oracle = syntax::oracle();
    let parse = oracle.parse(syntax::tokenize(data));

    for report in &parse.reports {
        let file = report::File {
            island: "".into(),
            path: "".into(),
            source: data.into(),
        };

        println!("{report}", report = report.with(&file));
    }

    if let Ok("" | "0" | "false") = env::var("FUZZ_PARSER_SAVE_VALID").as_deref() {
        return Corpus::Keep;
    }

    yansi::whenever(yansi::Condition::TTY_AND_COLOR);

    let Ok(node) = parse.result() else {
        return Corpus::Reject;
    };

    print!("found a valid parse!");

    let syntax = format!("{node:#?}", node = *node);

    let syntax_hash = {
        let mut hasher = hash::DefaultHasher::new();
        syntax.hash(&mut hasher);
        hasher.finish()
    };

    let base_file = format!("{syntax_hash:016x}");

    let (data_file, syntax_file) = {
        let root = path::Path::new("cab-syntax/test/data");
        fs::create_dir_all(root).unwrap();

        (
            root.join(base_file.clone() + ".cab"),
            root.join(base_file.clone() + ".expect"),
        )
    };

    if data_file.exists() {
        println!(
            " seems like it was already known before, skipping writing {name}",
            name = base_file.yellow().bold()
        );

        Corpus::Reject
    } else {
        println!(" wrote it to {name}", name = base_file.green().bold());
        fs::write(data_file, data).unwrap();
        fs::write(syntax_file, syntax).unwrap();

        Corpus::Keep
    }
});
