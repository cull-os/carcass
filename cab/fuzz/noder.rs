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
   sync::Arc,
};

use cab::{
   runtime::value,
   syntax,
};
use libfuzzer_sys::{
   Corpus,
   fuzz_target,
};
use rpds::ListSync as List;
use ust::{
   report,
   style::StyledExt as _,
   terminal,
   write,
};

fuzz_target!(|source: &str| -> Corpus {
   let parse_oracle = syntax::ParseOracle::new();
   let parse = parse_oracle.parse(syntax::tokenize(source));

   let path = value::Path::new(Arc::new(value::path::standard()), List::new_sync());
   let source = report::PositionStr::new(source);

   let out = &mut terminal::stdout();

   let save_valid = env::var_os("FUZZ_NODER_SAVE_VALID").is_some_and(|value| value != "0");
   let Ok(expression) = parse.extractlnln(out, &path, &source) else {
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
      let root = env::current_dir().unwrap();
      let root = root.parent().unwrap().join("target").join("cab-noder-fuzz");

      fs::create_dir_all(&root).unwrap();

      (
         root.join(base_file.clone() + ".cab"),
         root.join(base_file.clone() + ".expect"),
      )
   };

   let result = if source_file.exists() {
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
   };

   ust::Write::finish(out).unwrap();

   result
});
