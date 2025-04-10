#[derive(num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum Operation {
    // Small numbers represented as 1 u8 in vu128 are [0, 2**7) so starting the operation at that increses our chances
    // of Thunk::read_operation panicking if we ever write wrong code.
    Constant = 2u8.pow(7),
}
