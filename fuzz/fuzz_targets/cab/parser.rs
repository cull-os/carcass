#![no_main]

use std::{
    env,
    fs,
    hash::{
        self,
        Hash as _,
        Hasher as _,
    },
    path::Path,
    sync::Arc,
};

use cab::{
    island,
    syntax,
};
use libfuzzer_sys::{
    Corpus,
    fuzz_target,
};
use yansi::Paint as _;

fuzz_target!(|source: &str| -> Corpus {
    let oracle = syntax::oracle();
    let parse = oracle.parse(syntax::tokenize(source));

    let island: Arc<dyn island::Leaf> = Arc::new(island::blob(source.to_owned()));

    for report in &parse.reports {
        println!("{report}", report = report.with(island::display!(island), source));
    }

    let Ok("true" | "1") = env::var("FUZZ_PARSER_SAVE_VALID").as_deref() else {
        return Corpus::Keep;
    };

    yansi::whenever(yansi::Condition::TTY_AND_COLOR);

    let Ok(node) = parse.result() else {
        return Corpus::Reject;
    };

    print!("found a valid parse!");

    let display = format!("{node:#?}", node = *node);

    let display_hash = {
        let mut hasher = hash::DefaultHasher::new();
        display.hash(&mut hasher);
        hasher.finish()
    };

    let base_file = format!("{display_hash:016x}");

    let (source_file, display_file) = {
        let root = Path::new("cab-syntax/test/data");
        fs::create_dir_all(root).unwrap();

        (
            root.join(base_file.clone() + ".cab"),
            root.join(base_file.clone() + ".expect"),
        )
    };

    if source_file.exists() {
        println!(
            " seems like it was already known before, skipping writing {name}",
            name = base_file.yellow().bold()
        );

        Corpus::Reject
    } else {
        println!(" wrote it to {name}", name = base_file.green().bold());
        fs::write(source_file, source).unwrap();
        fs::write(display_file, display).unwrap();

        Corpus::Keep
    }
});
