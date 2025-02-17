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
    sync::{
        Arc,
        LazyLock,
    },
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

static RUNTIME: LazyLock<tokio::runtime::Runtime> =
    LazyLock::new(|| tokio::runtime::Builder::new_current_thread().build().unwrap());

fuzz_target!(|data: &str| -> Corpus {
    let oracle = syntax::oracle();
    let parse = oracle.parse(syntax::tokenize(data));

    let island: Arc<dyn island::Leaf> = Arc::new(island::blob(data.to_owned()));

    for report in &parse.reports {
        println!("{report}", report = RUNTIME.block_on(report.with(island.clone())));
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
