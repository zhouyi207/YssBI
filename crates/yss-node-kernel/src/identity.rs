use std::fmt;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum InvalidKernelIdentity {
    #[error("kernel identity is empty")]
    Empty,
    #[error("kernel identity has surrounding whitespace")]
    SurroundingWhitespace,
    #[error("kernel identity contains a NUL")]
    Nul,
}

fn validate(value: &str) -> Result<(), InvalidKernelIdentity> {
    if value.is_empty() {
        return Err(InvalidKernelIdentity::Empty);
    }
    if value.trim() != value {
        return Err(InvalidKernelIdentity::SurroundingWhitespace);
    }
    if value.contains('\0') {
        return Err(InvalidKernelIdentity::Nul);
    }
    Ok(())
}

macro_rules! kernel_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Box<str>);

        impl $name {
            pub fn new(value: Box<str>) -> Result<Self, InvalidKernelIdentity> {
                validate(&value)?;
                Ok(Self(value))
            }

            pub fn from_existing(value: Box<str>) -> Self {
                debug_assert!(validate(&value).is_ok());
                Self(value)
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

kernel_id!(KernelId);
kernel_id!(KernelParameterKey);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct KernelFingerprint([u8; 32]);

impl KernelFingerprint {
    pub const fn from_bytes(value: [u8; 32]) -> Self {
        Self(value)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}
