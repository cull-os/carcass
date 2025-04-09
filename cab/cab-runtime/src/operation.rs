#[derive(num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum Operation {
    Constant = 2u8.pow(7) + 1,
}
