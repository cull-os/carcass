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
   format::{
      self,
      style::StyleExt as _,
   },
   island,
   report,
   syntax,
};
use libfuzzer_sys::{
   Corpus,
   fuzz_target,
};

fuzz_target!(|source: &str| -> Corpus {
   let save_valid = matches!(
      env::var("FUZZ_NODER_SAVE_VALID").as_deref(),
      Ok("true" | "1")
   );

   let oracle = syntax::oracle();
   let parse = oracle.parse(syntax::tokenize(source));

   let island: Arc<dyn island::Leaf> = Arc::new(island::blob(source.to_owned()));

   format::init();

   let node = match parse.result() {
      Ok(node) => node,

      Err(reports) => {
         let source = report::PositionStr::new(source);

         for report in reports {
            println!(
               "{report}",
               report = report.with(island::display!(island), &source)
            );
         }

         return if save_valid {
            Corpus::Reject
         } else {
            Corpus::Keep
         };
      },
   };

   if !save_valid {
      return Corpus::Keep;
   }

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
