#![cfg_attr(doc, doc = include_str!("README.md"))]
#![no_std]
#![feature(impl_trait_in_assoc_type, gen_blocks)]
#![expect(clippy::undocumented_unsafe_blocks)]

use core::{
   marker,
   ptr,
};

use derive_more::Display;

pub type VectorEntry = (Result<VectorKey, usize>, usize);
pub type VectorEntryRaw = (usize, usize);

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, num_enum::TryFromPrimitive)]
#[repr(usize)]
pub enum VectorKey {
   /// Entry signaling the end of [`Vector`].
   ///
   /// The end marker is not just a key, it also has a value.
   ///
   /// The value is undefined, but it is commonly zero. Still, do not rely on
   /// it.
   ///
   /// We can verify this using Linux's `/proc/self/auxv`:
   ///
   /// ```shell
   /// od --format x8 /proc/self/auxv
   /// # 0000000 0000000000000021 00007f07987cd000
   /// # 0000020 0000000000000033 0000000000000d30
   /// # 0000040 0000000000000010 00000000178bfbff
   /// # 0000060 0000000000000006 0000000000001000
   /// # 0000100 0000000000000011 0000000000000064
   /// # 0000120 0000000000000003 000056192e789040
   /// # 0000140 0000000000000004 0000000000000038
   /// # 0000160 0000000000000005 000000000000000e
   /// # 0000200 0000000000000007 00007f07987cf000
   /// # 0000220 0000000000000008 0000000000000000
   /// # 0000240 0000000000000009 000056192e8db090
   /// # 0000260 000000000000000b 00000000000003e9
   /// # 0000300 000000000000000c 00000000000003e9
   /// # 0000320 000000000000000d 0000000000000064
   /// # 0000340 000000000000000e 0000000000000064
   /// # 0000360 0000000000000017 0000000000000000
   /// # 0000400 0000000000000019 00007ffdb1c0d729
   /// # 0000420 000000000000001a 0000000000000002
   /// # 0000440 000000000000001f 00007ffdb1c11fda
   /// # 0000460 000000000000000f 00007ffdb1c0d739
   /// # 0000500 000000000000001b 000000000000001c
   /// # 0000520 000000000000001c 0000000000000020
   /// # 0000540 0000000000000000 0000000000000000 (!!!)
   /// # 0000560
   /// ```
   ///
   /// The above snippet was done on a 64 bit system. On a 32 bit one, you'd do
   /// `--format x4`.
   ///
   /// Referred to as `AT_NULL` in [`libc`].
   #[display("AT_NULL")]
   #[doc(alias = "AT_NULL")]
   End                                      = 0,
   /// Entry should be ignored.
   ///
   /// Referred to as `AT_IGNORE` in [`libc`].
   #[display("AT_IGNORE")]
   #[doc(alias = "AT_IGNORE")]
   Ignored                                  = 1,
   /// Program's file descriptor.
   ///
   /// Referred to as `AT_EXECFD` in [`libc`].
   #[display("AT_EXECFD")]
   #[doc(alias = "AT_EXECFD")]
   Fd                                       = 2,
   /// Pointer to the program's header table.
   ///
   /// Referred to as `AT_PHDR` in [`libc`].
   #[display("AT_PHDR")]
   #[doc(alias = "AT_PHDR")]
   ProgramHeadersPointer                    = 3,
   /// Size of the [`ProgramHeadersPointer`] entry value in bytes.
   ///
   /// Referred to as `AT_PHENT` in [`libc`].
   #[display("AT_PHENT")]
   #[doc(alias = "AT_PHENT")]
   ProgramHeadersSize                       = 4,
   /// The total count of of program headers.
   ///
   /// Referred to as `AT_PHNUM` in [`libc`].
   #[display("AT_PHNUM")]
   #[doc(alias = "AT_PHNUM")]
   ProgramHeadersLength                     = 5,
   /// System page size in bytes.
   ///
   /// Referred to as `AT_PAGESZ` in [`libc`].
   #[display("AT_PAGESZ")]
   #[doc(alias = "AT_PAGESZ")]
   PageSize                                 = 6,
   /// Pointer to the base of the interpreter.
   ///
   /// Referred to as `AT_BASE` in [`libc`].
   #[display("AT_BASE")]
   #[doc(alias = "AT_BASE")]
   InterpreterBasePointer                   = 7,
   /// Flags.
   ///
   /// Referred to as `AT_FLAGS` in [`libc`].
   #[display("AT_FLAGS")]
   #[doc(alias = "AT_FLAGS")]
   Flags                                    = 8,
   /// Program's entry point.
   ///
   /// Basically where the interpreter should transfer control to.
   ///
   /// Referred to as `AT_ENTRY` in [`libc`].
   #[display("AT_ENTRY")]
   #[doc(alias = "AT_ENTRY")]
   Entrypoint                               = 9,
   /// Program is not ELF.
   ///
   /// The value of is non-zero if the program is in another
   /// format than ELF, for example in the old COFF format.
   ///
   /// Referred to as `AT_NOTELF` in [`libc`].
   #[display("AT_NOTELF")]
   #[doc(alias = "AT_NOTELF")]
   NotElf                                   = 10,
   /// Real UID.
   ///
   /// Referred to as `AT_UID` in [`libc`].
   #[display("AT_UID")]
   #[doc(alias = "AT_UID")]
   RealUid                                  = 11,
   /// Effective UID.
   ///
   /// Referred to as `AT_EUID` in [`libc`].
   #[display("AT_EUID")]
   #[doc(alias = "AT_EUID")]
   EffectiveUid                             = 12,
   /// Real GID.
   ///
   /// Referred to as `AT_GID` in [`libc`].
   #[display("AT_GID")]
   #[doc(alias = "AT_GID")]
   RealGid                                  = 13,
   /// Effective GID.
   ///
   /// Referred to as `AT_EGID` in [`libc`].
   #[display("AT_EGID")]
   #[doc(alias = "AT_EGID")]
   EffectiveGid                             = 14,
   /// String identifying the ELF's target platform.
   ///
   /// Referred to as `AT_PLATFORM` in [`libc`].
   #[display("AT_PLATFORM")]
   #[doc(alias = "AT_PLATFORM")]
   Platform                                 = 15,
   /// Arch-dependent hints about processor capabilities.
   ///
   /// Referred to as `AT_HWCAP` in [`libc`].
   #[display("AT_HWCAP")]
   #[doc(alias = "AT_HWCAP")]
   HardwareCapability                       = 16,
   /// Frequency of [`times()`](https://man7.org/linux/man-pages/man2/times.2.html).
   ///
   /// Referred to as `AT_CLKTCK` in [`libc`].
   #[display("AT_CLKTCK")]
   #[doc(alias = "AT_CLKTCK")]
   ClockTickFrequency                       = 17,
   /// Used FPU control word.
   ///
   /// Referred to as `AT_FPUCW` in [`libc`].
   #[display("AT_FPUCW")]
   #[doc(alias = "AT_FPUCW")]
   FpuControlWord                           = 18,
   /// Data cache block size in bytes.
   ///
   /// Referred to as `AT_DCACHEBSIZE` in [`libc`].
   #[display("AT_DCACHEBSIZE")]
   #[doc(alias = "AT_DCACHEBSIZE")]
   DataCacheBlockSize                       = 19,
   /// Instruction cache block size in bytes.
   ///
   /// Referred to as `AT_ICACHEBSIZE` in [`libc`].
   #[display("AT_ICACHEBSIZE")]
   #[doc(alias = "AT_ICACHEBSIZE")]
   InstructionCacheBlockSize                = 20,
   /// Unified cache block size in bytes.
   ///
   /// Referred to as `AT_UCACHEBSIZE` in [`libc`].
   #[display("AT_UCACHEBSIZE")]
   #[doc(alias = "AT_UCACHEBSIZE")]
   UnifiedCacheBlockSize                    = 21,
   /// A special ignored value for the `PowerPC` architecture, used by the
   /// kernel to control the interpretation of the auxiliary vector.
   ///
   /// Must be >16.
   ///
   /// Referred to as `AT_IGNOREPPC` in [`libc`].
   #[display("AT_IGNOREPPC")]
   #[doc(alias = "AT_IGNOREPPC")]
   IgnorePowerPC                            = 22,
   /// A boolean indicating whether `exec` was setuid or something
   /// similar to it
   ///
   /// Referred to as `AT_SECURE` in [`libc`].
   #[display("AT_SECURE")]
   #[doc(alias = "AT_SECURE")]
   Secure                                   = 23,
   /// String identifying real platforms.
   ///
   /// Referred to as `AT_BASE_PLATFORM` in [`libc`].
   #[display("AT_BASE_PLATFORM")]
   #[doc(alias = "AT_BASE_PLATFORM")]
   BasePlatform                             = 24,
   /// Address of 16, random bytes.
   ///
   /// Referred to as `AT_RANDOM` in [`libc`].
   #[display("AT_RANDOM")]
   #[doc(alias = "AT_RANDOM")]
   Random                                   = 25,
   /// Extension of [`HardwareCabability`].
   ///
   /// Referred to as `AT_HWCAP2` in [`libc`].
   #[display("AT_HWCAP2")]
   #[doc(alias = "AT_HWCAP2")]
   HardwareCapability2                      = 26,
   /// Restartable Sequences (rseq) supported feature size.
   ///
   /// Referred to as `AT_RSEQ_FEATURE_SIZE` in [`libc`].
   #[display("AT_RSEQ_FEATURE_SIZE")]
   #[doc(alias = "AT_RSEQ_FEATURE_SIZE")]
   RestartableSequencesSupportedFeatureSize = 27,
   /// Restartable Sequences (rseq) allocation alignment.
   ///
   /// Referred to as `AT_RSEQ_ALIGN` in [`libc`].
   #[display("AT_RSEQ_ALIGN")]
   #[doc(alias = "AT_RSEQ_ALIGN")]
   RestartableSequencesAllocationAlignment  = 28,
   /// Extension of [`HardwareCabability`].
   ///
   /// Referred to as `AT_HWCAP3` in [`libc`].
   #[display("AT_HWCAP3")]
   #[doc(alias = "AT_HWCAP3")]
   HardwareCapability3                      = 29,
   /// ///
   /// Referred to as `AT_HWCAP4` in [`libc`].
   /// Extension of [`HardwareCabability`].
   #[doc(alias = "AT_HWCAP4")]
   HardwareCapability4                      = 30,
   /// Pointer to the null-terminated filename of the executable.
   ///
   /// Referred to as `AT_EXECFN` in [`libc`].
   #[display("AT_EXECFN")]
   #[doc(alias = "AT_EXECFN")]
   FilenamePointer                          = 31,
   /// Pointer to the global system page used for system calls and other.
   /// nice things.
   ///
   /// Referred to as `AT_SYSINFO` in [`libc`].
   #[display("AT_SYSINFO")]
   #[doc(alias = "AT_SYSINFO")]
   SystemCallPagePointer                    = 32,
   /// Pointer to the system call page [virtual dynamic shared object (VDSO)](https://man7.org/linux/man-pages/man7/vdso.7.html).
   ///
   /// Referred to as `AT_SYSINFO_EHDR` in [`libc`].
   #[display("AT_SYSINFO_EHDR")]
   #[doc(alias = "AT_SYSINFO_EHDR")]
   SystemCallPageElfHeaderPointer           = 33,
   /// Shapes of caches.
   ///
   /// Bits 0-3 contains associativity; bits 4-7 contains
   /// log2 of line size; mask those to get cache size.
   ///
   ///
   /// Applies to all `CacheShape`.
   ///
   /// Referred to as `AT_L1I_CACHESHAPE` in [`libc`].
   #[display("AT_L1I_CACHESHAPE")]
   #[doc(alias = "AT_L1I_CACHESHAPE")]
   L1ICacheShape                            = 34,
   /// Referred to as `AT_L1D_CACHESHAPE` in [`libc`].
   #[display("AT_L1D_CACHESHAPE")]
   #[doc(alias = "AT_L1D_CACHESHAPE")]
   L1DCacheShape                            = 35,
   /// Referred to as `AT_L2_CACHESHAPE` in [`libc`].
   #[display("AT_L2_CACHESHAPE")]
   #[doc(alias = "AT_L2_CACHESHAPE")]
   L2CacheShape                             = 36,
   /// Referred to as `AT_L3_CACHESHAPE` in [`libc`].
   #[display("AT_L3_CACHESHAPE")]
   #[doc(alias = "AT_L3_CACHESHAPE")]
   L3CacheShape                             = 37,

   // ?? 38 ??
   // ?? 39 ??
   /// Shapes of the caches, with more room to describe them.
   ///
   /// `Geometry` are comprised of cache line size in bytes in the bottom 16
   /// bits and the cache associativity in the next 16 bits.
   ///
   /// Referred to as `AT_L1I_CACHESIZE` in [`libc`].
   #[display("AT_L1I_CACHESIZE")]
   #[doc(alias = "AT_L1I_CACHESIZE")]
   L1ICacheSize                             = 40,
   /// Referred to as `AT_L1I_CACHEGEOMETRY` in [`libc`].
   #[display("AT_L1I_CACHEGEOMETRY")]
   #[doc(alias = "AT_L1I_CACHEGEOMETRY")]
   L1ICacheGeometry                         = 41,
   /// Referred to as `AT_L1D_CACHESIZE` in [`libc`].
   #[display("AT_L1D_CACHESIZE")]
   #[doc(alias = "AT_L1D_CACHESIZE")]
   L1DCacheSize                             = 42,
   /// Referred to as `AT_L1D_CACHEGEOMETRY` in [`libc`].
   #[display("AT_L1D_CACHEGEOMETRY")]
   #[doc(alias = "AT_L1D_CACHEGEOMETRY")]
   L1DCacheGeometry                         = 43,
   /// Referred to as `AT_L2_CACHESIZE` in [`libc`].
   #[display("AT_L2_CACHESIZE")]
   #[doc(alias = "AT_L2_CACHESIZE")]
   L2CacheSize                              = 44,
   /// Referred to as `AT_L2_CACHEGEOMETRY` in [`libc`].
   #[display("AT_L2_CACHEGEOMETRY")]
   #[doc(alias = "AT_L2_CACHEGEOMETRY")]
   L2CacheGeometry                          = 45,
   /// Referred to as `AT_L3_CACHESIZE` in [`libc`].
   #[display("AT_L3_CACHESIZE")]
   #[doc(alias = "AT_L3_CACHESIZE")]
   L3CacheSize                              = 46,
   /// Referred to as `AT_L3_CACHEGEOMETRY` in [`libc`].
   #[display("AT_L3_CACHEGEOMETRY")]
   #[doc(alias = "AT_L3_CACHEGEOMETRY")]
   L3CacheGeometry                          = 47,

   // ?? 48 ??
   // ?? 49 ??
   // ?? 50 ??
   /// Stack needed for signal delivery.
   ///
   /// Referred to as `AT_MINSIGSTKSZ` in [`libc`].
   #[display("AT_MINSIGSTKSZ")]
   #[doc(alias = "AT_MINSIGSTKSZ")]
   SignalStackSizeMinimum                   = 51,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vector<'elf> {
   start:    *mut (usize, usize),
   _phantom: marker::PhantomData<&'elf [(usize, usize)]>,
}

macro_rules! iter_raw_impl {
   ($self:ident $($borrow:tt)*) => {
      gen move {
         let mut entry = $self.start;

         loop {
            // Found the end entry. Return it and end the generator.
            if unsafe { *entry }.0 == 0 {
               yield unsafe { $($borrow)* *entry };
               return;
            }

            yield unsafe { $($borrow)* *entry };

            entry = unsafe { entry.offset(1) };
         }
      }
   };
}

impl Vector<'_> {
   /// Iterate over the entries, copying them and turning the key into
   /// [`VectorKey`].
   ///
   /// # Safety
   ///
   /// Caller must ensure that no other [`Vector`] is iterating mutably
   /// over the same auxiliary vector address.
   pub unsafe fn iter(&self) -> impl Iterator<Item = VectorEntry> {
      gen move {
         let mut entry = self.start;

         loop {
            let (key, value) = unsafe { *entry };
            let key = VectorKey::try_from(key).map_err(|error| error.number);

            // Found the end entry. Return it and end the generator.
            if let Ok(VectorKey::End) = key {
               yield (key, value);
               return;
            }

            yield (key, value);

            entry = unsafe { entry.offset(1) };
         }
      }
   }

   /// Iterate over the raw entries, without copying them.
   ///
   /// # Safety
   ///
   /// Caller must ensure that no other [`Vector`] is iterating mutably
   /// over the same auxiliary vector address.
   pub unsafe fn iter_raw(&self) -> impl Iterator<Item = &VectorEntryRaw> {
      iter_raw_impl!(self &)
   }

   /// Iterate over the raw entries with the ability to mutate them, without
   /// copying them.
   ///
   /// # Example
   ///
   /// ```no_run
   /// # use auxvec::{Vector, VectorKey};
   /// # #[cfg(all(unix, not(target_os = "macos"), not(target_os = "ios")))] {
   /// // SAFETY: This program is an ELF file.
   /// let mut aux = unsafe { Vector::chase_environ() };
   /// # }
   /// # let mut aux = unsafe { Vector::from(unimplemented!()) };
   ///
   /// // SAFETY: No other thread is accessig this program's environ.
   /// for (key, value) in unsafe { aux.iter_raw_mut() } {
   ///    if VectorKey::try_from(*key) == Ok(VectorKey::NotElf) {
   ///       *value = 0; // Haha! You are ELF now.
   ///    }
   /// }
   /// ```
   ///
   /// # Safety
   ///
   /// Caller must ensure that no other [`Vector`] is iterating mutably
   /// over the same auxiliary vector address.
   ///
   /// The side effects of changing auxiliary vector values is also your
   /// concern; not within the safety scopes of the Rust programming language.
   pub unsafe fn iter_raw_mut(&mut self) -> impl Iterator<Item = &mut VectorEntryRaw> {
      iter_raw_impl!(self &mut)
   }
}

impl Vector<'_> {
   /// Creates an [`Vector`] by chasing the
   /// end of the current processes `environ`.
   ///
   /// # Safety
   ///
   /// Caller must ensure that the end of the environ must be the start of the
   /// auxiliary vector. This is the case when targeting a UNIX platform that
   /// isn't Darwin or iOS as they uses Mach-O rather than ELF.
   #[must_use]
   #[cfg(all(unix, not(target_os = "macos"), not(target_os = "ios")))]
   pub unsafe fn chase_environ() -> Vector<'static> {
      unsafe extern "C" {
         /// A pointer to a sequence of pointers that point to null
         /// terminated strings of the format: `"NAME=value\0"`.
         ///
         /// The end of the environ is markted by a null byte.
         static environ: *const *const u8;
      }

      let mut environ_entry: *const *const u8 = unsafe { environ };

      while !unsafe { *environ_entry }.is_null() {
         // Skip the pointers to "NAME=value\0" strings.
         environ_entry = unsafe { environ_entry.offset(1) };
      }
      // It now points at the null at the end of environ.

      Vector {
         // Offset by one to get the start of the auxiliary vector.
         start:    unsafe { environ_entry.offset(1) }.cast(),
         _phantom: marker::PhantomData,
      }
   }

   /// Creates an [`Vector`] with the given reference to the first entry.
   ///
   /// # Safety
   ///
   /// Caller must ensure that the end of the environ must be the start of the
   /// auxiliary vector. This is the case when targeting a UNIX platform that
   /// isn't Darwin or iOS as they uses Mach-O rather than ELF.
   #[must_use]
   pub unsafe fn from(start: &VectorEntryRaw) -> Vector<'_> {
      Vector {
         start:    ptr::from_ref(start).cast_mut(),
         _phantom: marker::PhantomData,
      }
   }
}
