#![no_main]

use std::{
   env,
   fmt::Write as _,
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
use ust::{
   report,
   style::StyledExt as _,
   terminal,
   write,
};

fuzz_target!(|source: &str| -> Corpus {
   let parse_oracle = syntax::parse_oracle();
   let parse = parse_oracle.parse(syntax::tokenize(source));

   let island: Arc<dyn island::Leaf> = Arc::new(island::blob(source.to_owned()));
   let source = report::PositionStr::new(source);

   let save_valid = matches!(
      env::var("FUZZ_NODER_SAVE_VALID").as_deref(),
      Ok("true" | "1"),
   );

   let out = &mut terminal::stdout();

   let Ok(expression) = parse.extractlnln(out, &island::display!(island), &source) else {
      return if save_valid {
         Corpus::Reject
      } else {
         Corpus::Keep
      };
   };

   if !save_valid {
      return Corpus::Keep;
   }

   write!(out, "found a valid parse!").unwrap();

   let display = format!("{node:#?}", node = *expression);

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
      write!(
         out,
         " seems like it was already known before, skipping writing "
      )
      .unwrap();
      write(out, &base_file.yellow().bold()).unwrap();

      Corpus::Reject
   } else {
      fs::write(source_file, *source).unwrap();
      fs::write(display_file, display).unwrap();

      write!(out, " wrote it to ").unwrap();
      write(out, &base_file.green().bold()).unwrap();

      Corpus::Keep
   }
});
